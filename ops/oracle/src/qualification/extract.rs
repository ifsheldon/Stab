use std::collections::BTreeSet;
use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

const MAX_PYTHON_INPUT_BYTES: usize = 16 << 20;
const MAX_PYTHON_OUTPUT_BYTES: usize = 1 << 20;
const MAX_PYTHON_OUTPUT_FIELD_BYTES: usize = 2_048;
const MAX_PYTHON_FILES: usize = 512;
#[cfg(test)]
const MAX_EXTRACTED_CASES: usize = 8_192;
const MAX_SOURCE_PATH_BYTES: usize = 512;
const WORD_SIZES: [u16; 3] = [64, 128, 256];
pub(super) const PYTHON_AST_VERSION: &str = "3.14.6";

const PYTHON_AST_SCRIPT: &str = r#"
import ast
import hashlib
import itertools
import json
import sys

payload = json.load(sys.stdin)
records = []
max_cases = payload["max_cases"]
max_output_bytes = payload["max_output_bytes"]
max_field_bytes = payload["max_field_bytes"]
output_bytes = 128

def add_record(record):
    global output_bytes
    if len(records) >= max_cases:
        raise ValueError(f"Python test extraction exceeds {max_cases} cases")
    for field in ("path", "symbol", "subcase"):
        value = record[field]
        if value is not None and len(value.encode("utf-8")) > max_field_bytes:
            raise ValueError(f"Python test extraction field {field} exceeds {max_field_bytes} bytes")
    encoded_bytes = len(json.dumps(record, separators=(",", ":")).encode("utf-8")) + 1
    if output_bytes + encoded_bytes > max_output_bytes:
        raise ValueError(f"Python test extraction exceeds {max_output_bytes} output bytes")
    output_bytes += encoded_bytes
    records.append(record)

class TestVisitor(ast.NodeVisitor):
    def __init__(self, path):
        self.path = path
        self.classes = []

    def visit_ClassDef(self, node):
        self.classes.append(node.name)
        for child in node.body:
            self.visit(child)
        self.classes.pop()

    def visit_FunctionDef(self, node):
        if node.name.startswith("test_"):
            symbol = ".".join([*self.classes, node.name])
            cases = parameter_cases(node)
            if cases is None:
                add_record({
                    "path": self.path,
                    "symbol": symbol,
                    "subcase": None,
                    "dynamic_parameters": False,
                    "line": node.lineno,
                })
            else:
                for subcase, dynamic in cases:
                    add_record({
                        "path": self.path,
                        "symbol": symbol,
                        "subcase": subcase,
                        "dynamic_parameters": dynamic,
                        "line": node.lineno,
                    })

    def visit_AsyncFunctionDef(self, node):
        self.visit_FunctionDef(node)

def is_parametrize(decorator):
    func = decorator.func if isinstance(decorator, ast.Call) else None
    return (
        isinstance(func, ast.Attribute)
        and func.attr == "parametrize"
        and isinstance(func.value, ast.Attribute)
        and func.value.attr == "mark"
        and isinstance(func.value.value, ast.Name)
        and func.value.value.id == "pytest"
    )

def node_digest(node):
    payload = ast.dump(node, annotate_fields=True, include_attributes=False)
    return hashlib.sha256(payload.encode("utf-8")).hexdigest()[:20]

def parameter_names(node):
    try:
        value = ast.literal_eval(node)
    except Exception:
        return "dynamic-names-" + node_digest(node)
    if isinstance(value, str):
        return ",".join(part.strip() for part in value.split(","))
    if isinstance(value, (list, tuple)) and all(isinstance(part, str) for part in value):
        return ",".join(value)
    return "dynamic-names-" + node_digest(node)

def expand_values(node):
    if isinstance(node, (ast.List, ast.Tuple, ast.Set)):
        if len(node.elts) > 1024:
            raise ValueError("pytest literal parameter expansion exceeds 1024 subcases")
        return [(node_digest(value), False) for value in node.elts]
    if isinstance(node, ast.Dict):
        keys = [value for value in node.keys if value is not None]
        if len(keys) > 1024:
            raise ValueError("pytest dictionary parameter expansion exceeds 1024 subcases")
        return [(node_digest(value), False) for value in keys]
    if (
        isinstance(node, ast.Call)
        and isinstance(node.func, ast.Name)
        and node.func.id == "range"
        and not node.keywords
    ):
        try:
            args = [ast.literal_eval(arg) for arg in node.args]
            values = range(*args)
        except Exception:
            values = None
        if values is not None:
            try:
                count = len(values)
            except OverflowError as ex:
                raise ValueError("pytest range parameter expansion exceeds 1024 subcases") from ex
            if count > 1024:
                raise ValueError("pytest range parameter expansion exceeds 1024 subcases")
            return [
                (hashlib.sha256(repr(value).encode("utf-8")).hexdigest()[:20], False)
                for value in values
            ]
    if (
        isinstance(node, ast.Call)
        and isinstance(node.func, ast.Attribute)
        and isinstance(node.func.value, ast.Name)
        and node.func.value.id == "itertools"
        and node.func.attr == "product"
        and not node.keywords
    ):
        factors = [expand_values(arg) for arg in node.args]
        if factors and all(not dynamic for factor in factors for _, dynamic in factor):
            product = []
            for values in itertools.product(*factors):
                payload = "\0".join(value for value, _ in values)
                product.append((hashlib.sha256(payload.encode("ascii")).hexdigest()[:20], False))
                if len(product) > 1024:
                    raise ValueError("pytest parameter expansion exceeds 1024 subcases")
            return product
    return [(node_digest(node), True)]

def parameter_cases(node):
    dimensions = []
    for decorator in node.decorator_list:
        if not is_parametrize(decorator):
            continue
        if len(decorator.args) < 2:
            dimensions.append(("invalid-parametrize", [("invalid-parametrize", True)]))
            continue
        names = parameter_names(decorator.args[0])
        if len(names.encode("utf-8")) + 128 > max_field_bytes:
            raise ValueError("pytest parameter names exceed the output field budget")
        values = expand_values(decorator.args[1])
        dimensions.append((names, values))
    if not dimensions:
        return None
    return iter_parameter_cases(dimensions)

def iter_parameter_cases(dimensions):
    factors = [values for _, values in dimensions]
    for index, values in enumerate(itertools.product(*factors)):
        if index >= 1024:
            raise ValueError("pytest parameter expansion exceeds 1024 subcases")
        parts = []
        dynamic_case = False
        for (names, _), (digest, dynamic) in zip(dimensions, values):
            parts.append(
                ("dynamic-parameter-family" if dynamic else "parameter-subcase")
                + ":" + names + ":sha256=" + digest
            )
            dynamic_case = dynamic_case or dynamic
        subcase = ";".join(parts)
        if len(subcase.encode("utf-8")) > max_field_bytes:
            raise ValueError("pytest parameter subcase exceeds the output field budget")
        yield subcase, dynamic_case

for source in payload["sources"]:
    tree = ast.parse(source["content"], filename=source["path"])
    TestVisitor(source["path"]).visit(tree)

records.sort(key=lambda item: (item["path"], item["line"], item["symbol"], item["subcase"] or ""))
json.dump({"python_version": f"{sys.version_info.major}.{sys.version_info.minor}.{sys.version_info.micro}", "records": records}, sys.stdout, separators=(",", ":"))
"#;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CppTestDeclaration {
    pub(super) macro_name: &'static str,
    pub(super) symbol: String,
    pub(super) subcase: Option<String>,
    pub(super) line: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PythonTestDeclaration {
    pub(super) path: String,
    pub(super) symbol: String,
    pub(super) subcase: Option<String>,
    pub(super) dynamic_parameters: bool,
    pub(super) line: u32,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct PythonSource<'a> {
    pub(super) path: &'a str,
    pub(super) content: &'a str,
}

#[derive(Debug, Error)]
pub(crate) enum ExtractionError {
    #[error("C++ source contains an unterminated {kind} starting at byte {offset}")]
    UnterminatedCppToken { kind: &'static str, offset: usize },

    #[error("C++ test macro {macro_name} at line {line} has malformed {field}")]
    MalformedCppMacro {
        macro_name: String,
        line: u32,
        field: &'static str,
    },

    #[error("Python source inventory has {actual} files; limit is {limit}")]
    TooManyPythonFiles { actual: usize, limit: usize },

    #[error("Python source inventory is {actual} bytes; limit is {limit}")]
    PythonInputTooLarge { actual: usize, limit: usize },

    #[error("Python source path {path:?} is invalid")]
    InvalidPythonPath { path: String },

    #[error("failed to serialize Python AST input: {0}")]
    SerializePythonInput(serde_json::Error),

    #[error("failed to execute Python AST extractor: {0}")]
    PythonProcess(Box<str>),

    #[error("Python AST extractor exited with {status}\nstdout:\n{stdout}\nstderr:\n{stderr}")]
    PythonFailed {
        status: String,
        stdout: Box<str>,
        stderr: Box<str>,
    },

    #[error("Python AST extractor output is not UTF-8")]
    NonUtf8PythonOutput,

    #[error("failed to parse Python AST extractor output: {0}")]
    ParsePythonOutput(serde_json::Error),

    #[error("Python AST extractor used version {actual}, expected {expected}")]
    WrongPythonVersion {
        actual: String,
        expected: &'static str,
    },

    #[error("Python AST extractor returned unknown path {0:?}")]
    UnknownPythonPath(String),

    #[error("extracted {actual} test cases; limit is {limit}")]
    TooManyCases { actual: usize, limit: usize },
}

#[derive(Serialize)]
struct PythonAstInput<'a> {
    path: &'a str,
    content: &'a str,
}

#[derive(Serialize)]
struct PythonAstInputEnvelope<'a> {
    sources: Vec<PythonAstInput<'a>>,
    max_cases: usize,
    max_output_bytes: usize,
    max_field_bytes: usize,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PythonAstOutputEnvelope {
    python_version: String,
    records: Vec<PythonAstOutput>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PythonAstOutput {
    path: String,
    symbol: String,
    subcase: Option<String>,
    dynamic_parameters: bool,
    line: u32,
}

#[cfg(test)]
pub(super) fn extract_cpp_test_cases(
    source: &str,
) -> Result<Vec<CppTestDeclaration>, ExtractionError> {
    extract_cpp_test_cases_bounded(source, MAX_EXTRACTED_CASES)
}

pub(super) fn extract_cpp_test_cases_bounded(
    source: &str,
    case_limit: usize,
) -> Result<Vec<CppTestDeclaration>, ExtractionError> {
    let masked = mask_cpp_non_code(source)?;
    let mut cases = Vec::new();
    let mut offset = 0usize;
    let mut line = 1u32;
    while offset < masked.len() {
        let Some(byte) = masked.get(offset).copied() else {
            break;
        };
        if !is_identifier_start(byte) {
            if byte == b'\n' {
                line = line.saturating_add(1);
            }
            offset += 1;
            continue;
        }
        let token_start = offset;
        offset = consume_identifier(&masked, offset);
        let Some(token) = source.get(token_start..offset) else {
            continue;
        };
        let macro_name = match token {
            "TEST" => "TEST",
            "TEST_F" => "TEST_F",
            "TYPED_TEST" => "TYPED_TEST",
            "TEST_EACH_WORD_SIZE_W" => "TEST_EACH_WORD_SIZE_W",
            _ => continue,
        };
        let mut cursor = skip_ascii_whitespace(&masked, offset);
        cursor = expect_byte(
            &masked,
            cursor,
            b'(',
            macro_name,
            line,
            "opening parenthesis",
        )?;
        let (suite, after_suite) =
            parse_identifier(source, &masked, cursor, macro_name, line, "suite")?;
        cursor = skip_ascii_whitespace(&masked, after_suite);
        cursor = expect_byte(&masked, cursor, b',', macro_name, line, "suite separator")?;
        cursor = skip_ascii_whitespace(&masked, cursor);
        let (name, after_name) =
            parse_identifier(source, &masked, cursor, macro_name, line, "name")?;
        cursor = skip_ascii_whitespace(&masked, after_name);

        if macro_name != "TEST_EACH_WORD_SIZE_W" {
            let _ = expect_byte(
                &masked,
                cursor,
                b')',
                macro_name,
                line,
                "closing parenthesis",
            )?;
            cases.push(CppTestDeclaration {
                macro_name,
                symbol: format!("{suite}.{name}"),
                subcase: None,
                line,
            });
        } else {
            let _ = expect_byte(&masked, cursor, b',', macro_name, line, "body separator")?;
            for word_size in WORD_SIZES {
                cases.push(CppTestDeclaration {
                    macro_name,
                    symbol: format!("{suite}.{name}_{word_size}"),
                    subcase: Some(format!("W={word_size}")),
                    line,
                });
            }
        }
        if cases.len() > case_limit {
            return Err(ExtractionError::TooManyCases {
                actual: cases.len(),
                limit: case_limit,
            });
        }
    }
    Ok(cases)
}

#[cfg(test)]
pub(super) fn extract_python_test_cases(
    sources: &[PythonSource<'_>],
    root: &Path,
) -> Result<Vec<PythonTestDeclaration>, ExtractionError> {
    extract_python_test_cases_bounded(sources, root, MAX_EXTRACTED_CASES)
}

pub(super) fn extract_python_test_cases_bounded(
    sources: &[PythonSource<'_>],
    root: &Path,
    case_limit: usize,
) -> Result<Vec<PythonTestDeclaration>, ExtractionError> {
    if sources.len() > MAX_PYTHON_FILES {
        return Err(ExtractionError::TooManyPythonFiles {
            actual: sources.len(),
            limit: MAX_PYTHON_FILES,
        });
    }
    let mut known_paths = BTreeSet::new();
    let mut total_bytes = 0usize;
    let mut input = Vec::with_capacity(sources.len());
    for source in sources {
        if source.path.is_empty()
            || source.path.len() > MAX_SOURCE_PATH_BYTES
            || source.path.chars().any(char::is_control)
            || !known_paths.insert(source.path)
        {
            return Err(ExtractionError::InvalidPythonPath {
                path: source.path.to_string(),
            });
        }
        total_bytes = total_bytes
            .checked_add(source.path.len())
            .and_then(|value| value.checked_add(source.content.len()))
            .ok_or(ExtractionError::PythonInputTooLarge {
                actual: usize::MAX,
                limit: MAX_PYTHON_INPUT_BYTES,
            })?;
        input.push(PythonAstInput {
            path: source.path,
            content: source.content,
        });
    }
    if total_bytes > MAX_PYTHON_INPUT_BYTES {
        return Err(ExtractionError::PythonInputTooLarge {
            actual: total_bytes,
            limit: MAX_PYTHON_INPUT_BYTES,
        });
    }
    let stdin = serde_json::to_vec(&PythonAstInputEnvelope {
        sources: input,
        max_cases: case_limit,
        max_output_bytes: MAX_PYTHON_OUTPUT_BYTES,
        max_field_bytes: MAX_PYTHON_OUTPUT_FIELD_BYTES,
    })
    .map_err(ExtractionError::SerializePythonInput)?;
    let output = crate::run_process(
        Path::new("uv"),
        [
            "run",
            "--no-project",
            "--python",
            PYTHON_AST_VERSION,
            "python",
            "-I",
            "-c",
            PYTHON_AST_SCRIPT,
        ],
        &stdin,
        Some(root),
    )
    .map_err(|source| ExtractionError::PythonProcess(source.to_string().into_boxed_str()))?;
    if !output.success() {
        return Err(ExtractionError::PythonFailed {
            status: crate::process::display_status(output.status),
            stdout: output.stdout.render_for_diagnostics().into_boxed_str(),
            stderr: output.stderr.render_for_diagnostics().into_boxed_str(),
        });
    }
    let stdout = std::str::from_utf8(&output.stdout.bytes)
        .map_err(|_| ExtractionError::NonUtf8PythonOutput)?;
    let parsed: PythonAstOutputEnvelope =
        serde_json::from_str(stdout).map_err(ExtractionError::ParsePythonOutput)?;
    if parsed.python_version != PYTHON_AST_VERSION {
        return Err(ExtractionError::WrongPythonVersion {
            actual: parsed.python_version,
            expected: PYTHON_AST_VERSION,
        });
    }
    if parsed.records.len() > case_limit {
        return Err(ExtractionError::TooManyCases {
            actual: parsed.records.len(),
            limit: case_limit,
        });
    }
    let mut cases = Vec::with_capacity(parsed.records.len());
    for item in parsed.records {
        if !known_paths.contains(item.path.as_str()) {
            return Err(ExtractionError::UnknownPythonPath(item.path));
        }
        cases.push(PythonTestDeclaration {
            path: item.path,
            symbol: item.symbol,
            subcase: item.subcase,
            dynamic_parameters: item.dynamic_parameters,
            line: item.line,
        });
    }
    Ok(cases)
}

fn mask_cpp_non_code(source: &str) -> Result<Vec<u8>, ExtractionError> {
    let bytes = source.as_bytes();
    let mut masked = bytes.to_vec();
    let mut offset = 0usize;
    while offset < bytes.len() {
        match (bytes.get(offset), bytes.get(offset + 1)) {
            (Some(b'/'), Some(b'/')) => {
                let start = offset;
                offset += 2;
                while bytes.get(offset).is_some_and(|byte| *byte != b'\n') {
                    offset += 1;
                }
                blank_non_newlines(&mut masked, start, offset);
            }
            (Some(b'/'), Some(b'*')) => {
                let start = offset;
                offset += 2;
                loop {
                    match (bytes.get(offset), bytes.get(offset + 1)) {
                        (Some(b'*'), Some(b'/')) => {
                            offset += 2;
                            blank_non_newlines(&mut masked, start, offset);
                            break;
                        }
                        (Some(_), _) => offset += 1,
                        (None, _) => {
                            return Err(ExtractionError::UnterminatedCppToken {
                                kind: "block comment",
                                offset: start,
                            });
                        }
                    }
                }
            }
            (Some(b'R'), Some(b'"')) => {
                let start = offset;
                offset = consume_cpp_raw_string(bytes, offset)?;
                blank_non_newlines(&mut masked, start, offset);
            }
            (Some(b'"'), _) | (Some(b'\''), _) => {
                let start = offset;
                let quote = bytes.get(offset).copied().unwrap_or_default();
                offset = consume_cpp_quoted_literal(bytes, offset, quote)?;
                blank_non_newlines(&mut masked, start, offset);
            }
            (Some(_), _) => offset += 1,
            (None, _) => break,
        }
    }
    Ok(masked)
}

fn consume_cpp_quoted_literal(
    bytes: &[u8],
    start: usize,
    quote: u8,
) -> Result<usize, ExtractionError> {
    let mut offset = start + 1;
    let mut escaped = false;
    while let Some(byte) = bytes.get(offset).copied() {
        offset += 1;
        if escaped {
            escaped = false;
        } else if byte == b'\\' {
            escaped = true;
        } else if byte == quote {
            return Ok(offset);
        }
    }
    Err(ExtractionError::UnterminatedCppToken {
        kind: "quoted literal",
        offset: start,
    })
}

fn consume_cpp_raw_string(bytes: &[u8], start: usize) -> Result<usize, ExtractionError> {
    let delimiter_start = start + 2;
    let mut delimiter_end = delimiter_start;
    while let Some(byte) = bytes.get(delimiter_end).copied() {
        if byte == b'(' {
            break;
        }
        if delimiter_end - delimiter_start >= 16
            || byte.is_ascii_whitespace()
            || matches!(byte, b'\\' | b')')
        {
            return Err(ExtractionError::UnterminatedCppToken {
                kind: "raw string delimiter",
                offset: start,
            });
        }
        delimiter_end += 1;
    }
    if bytes.get(delimiter_end) != Some(&b'(') {
        return Err(ExtractionError::UnterminatedCppToken {
            kind: "raw string delimiter",
            offset: start,
        });
    }
    let delimiter = bytes
        .get(delimiter_start..delimiter_end)
        .unwrap_or_default();
    let mut offset = delimiter_end + 1;
    while let Some(byte) = bytes.get(offset).copied() {
        if byte == b')' {
            let suffix_start = offset + 1;
            let suffix_end = suffix_start.saturating_add(delimiter.len());
            if bytes.get(suffix_start..suffix_end) == Some(delimiter)
                && bytes.get(suffix_end) == Some(&b'"')
            {
                return Ok(suffix_end + 1);
            }
        }
        offset += 1;
    }
    Err(ExtractionError::UnterminatedCppToken {
        kind: "raw string",
        offset: start,
    })
}

fn blank_non_newlines(masked: &mut [u8], start: usize, end: usize) {
    if let Some(region) = masked.get_mut(start..end) {
        for byte in region {
            if *byte != b'\n' {
                *byte = b' ';
            }
        }
    }
}

fn consume_identifier(bytes: &[u8], mut offset: usize) -> usize {
    while bytes
        .get(offset)
        .is_some_and(|byte| is_identifier_continue(*byte))
    {
        offset += 1;
    }
    offset
}

fn parse_identifier<'a>(
    source: &'a str,
    masked: &[u8],
    offset: usize,
    macro_name: &str,
    line: u32,
    field: &'static str,
) -> Result<(&'a str, usize), ExtractionError> {
    if !masked
        .get(offset)
        .is_some_and(|byte| is_identifier_start(*byte))
    {
        return Err(ExtractionError::MalformedCppMacro {
            macro_name: macro_name.to_string(),
            line,
            field,
        });
    }
    let end = consume_identifier(masked, offset);
    let value = source
        .get(offset..end)
        .ok_or_else(|| ExtractionError::MalformedCppMacro {
            macro_name: macro_name.to_string(),
            line,
            field,
        })?;
    Ok((value, end))
}

fn expect_byte(
    bytes: &[u8],
    offset: usize,
    expected: u8,
    macro_name: &str,
    line: u32,
    field: &'static str,
) -> Result<usize, ExtractionError> {
    if bytes.get(offset) == Some(&expected) {
        Ok(offset + 1)
    } else {
        Err(ExtractionError::MalformedCppMacro {
            macro_name: macro_name.to_string(),
            line,
            field,
        })
    }
}

fn skip_ascii_whitespace(bytes: &[u8], mut offset: usize) -> usize {
    while bytes
        .get(offset)
        .is_some_and(|byte| byte.is_ascii_whitespace())
    {
        offset += 1;
    }
    offset
}

const fn is_identifier_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

const fn is_identifier_continue(byte: u8) -> bool {
    is_identifier_start(byte) || byte.is_ascii_digit()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpp_extractor_ignores_comments_literals_and_prefixes() {
        let source = r#"
// TEST(fake, line_comment) {}
const char *a = "TEST(fake, string) {}";
const char *b = R"tag(TEST(fake, raw_string) {})tag";
/* TEST(fake, block_comment) {} */
NOT_A_TEST(fake, prefix) {}
TEST(real_suite, real_case) {}
"#;
        let cases = extract_cpp_test_cases(source).expect("valid C++ fixture");
        assert_eq!(
            cases,
            vec![CppTestDeclaration {
                macro_name: "TEST",
                symbol: "real_suite.real_case".to_string(),
                subcase: None,
                line: 7,
            }]
        );
    }

    #[test]
    fn cpp_extractor_expands_word_size_macro_semantics() {
        let source = "TEST_EACH_WORD_SIZE_W(simd_bits, xor_tail, { ASSERT_TRUE(W); })\n";
        let cases = extract_cpp_test_cases(source).expect("valid word-size fixture");
        assert_eq!(cases.len(), 3);
        assert_eq!(
            cases.first().expect("first word size").symbol,
            "simd_bits.xor_tail_64"
        );
        assert_eq!(
            cases.get(1).expect("second word size").subcase.as_deref(),
            Some("W=128")
        );
        assert_eq!(
            cases.last().expect("last word size").symbol,
            "simd_bits.xor_tail_256"
        );
    }

    #[test]
    fn cpp_extractor_supports_fixture_and_typed_gtest_macros() {
        let source =
            "TEST_F(MyFixture, rejects_bad_input) {}\nTYPED_TEST(MyTypes, round_trips) {}\n";
        let cases = extract_cpp_test_cases(source).expect("valid GTest fixture");
        assert_eq!(cases.len(), 2);
        assert_eq!(
            cases.first().expect("fixture case").symbol,
            "MyFixture.rejects_bad_input"
        );
        assert_eq!(
            cases.last().expect("typed case").symbol,
            "MyTypes.round_trips"
        );
    }

    #[test]
    fn cpp_extractor_rejects_malformed_selected_macro() {
        let error = extract_cpp_test_cases("TEST(missing_comma) {}")
            .expect_err("selected malformed macro must fail");
        assert!(matches!(error, ExtractionError::MalformedCppMacro { .. }));
    }

    #[test]
    fn python_ast_extractor_ignores_strings_comments_and_nested_functions() {
        let source = r#"
"def test_string(): pass"
# def test_comment(): pass
def helper():
    def test_nested():
        pass

def test_module():
    pass

class TestGroup:
    async def test_method(self):
        pass
"#;
        let cases = extract_python_test_cases(
            &[PythonSource {
                path: "sample_test.py",
                content: source,
            }],
            Path::new("."),
        )
        .expect("valid Python fixture");
        assert_eq!(
            cases,
            vec![
                PythonTestDeclaration {
                    path: "sample_test.py".to_string(),
                    symbol: "test_module".to_string(),
                    subcase: None,
                    dynamic_parameters: false,
                    line: 8,
                },
                PythonTestDeclaration {
                    path: "sample_test.py".to_string(),
                    symbol: "TestGroup.test_method".to_string(),
                    subcase: None,
                    dynamic_parameters: false,
                    line: 12,
                },
            ]
        );
    }

    #[test]
    fn python_ast_extractor_expands_static_and_marks_dynamic_parameters() {
        let source = r#"
import pytest

@pytest.mark.parametrize("flag", [False, True])
def test_static(flag):
    pass

@pytest.mark.parametrize("item", CASES)
def test_dynamic(item):
    pass

@pytest.mark.parametrize("left", [0, 1])
@pytest.mark.parametrize("right", ["x", "y", "z"])
def test_stacked_static(left, right):
    pass

@pytest.mark.parametrize("left", [0, 1])
@pytest.mark.parametrize("right", CASES)
def test_stacked_dynamic(left, right):
    pass
"#;
        let cases = extract_python_test_cases(
            &[PythonSource {
                path: "parameters_test.py",
                content: source,
            }],
            Path::new("."),
        )
        .expect("valid parametrized Python fixture");
        let static_cases = cases
            .iter()
            .filter(|case| case.symbol == "test_static")
            .collect::<Vec<_>>();
        assert_eq!(static_cases.len(), 2);
        assert!(static_cases.iter().all(|case| {
            case.subcase
                .as_deref()
                .is_some_and(|subcase| subcase.starts_with("parameter-subcase:flag:sha256="))
                && !case.dynamic_parameters
        }));
        let dynamic = cases
            .iter()
            .find(|case| case.symbol == "test_dynamic")
            .expect("dynamic parameter family");
        assert!(dynamic.dynamic_parameters);
        assert!(
            dynamic
                .subcase
                .as_deref()
                .is_some_and(|subcase| subcase.starts_with("dynamic-parameter-family:item:sha256="))
        );
        let stacked_static = cases
            .iter()
            .filter(|case| case.symbol == "test_stacked_static")
            .collect::<Vec<_>>();
        assert_eq!(stacked_static.len(), 6);
        assert!(stacked_static.iter().all(|case| !case.dynamic_parameters));
        let stacked_dynamic = cases
            .iter()
            .filter(|case| case.symbol == "test_stacked_dynamic")
            .collect::<Vec<_>>();
        assert_eq!(stacked_dynamic.len(), 2);
        assert!(stacked_dynamic.iter().all(|case| case.dynamic_parameters));
    }

    #[test]
    fn extractors_reject_case_expansion_before_materializing_unbounded_work() {
        let cpp = "TEST(Suite, first) {}\nTEST(Suite, second) {}\n";
        let error = extract_cpp_test_cases_bounded(cpp, 1)
            .expect_err("global C++ case budget must fail during extraction");
        assert!(matches!(
            error,
            ExtractionError::TooManyCases { limit: 1, .. }
        ));

        let python = r#"
import pytest

@pytest.mark.parametrize("item", range(1_000_000_000))
def test_huge_range(item):
    pass
"#;
        let error = extract_python_test_cases(
            &[PythonSource {
                path: "huge_range_test.py",
                content: python,
            }],
            Path::new("."),
        )
        .expect_err("huge range must fail before materialization");
        assert!(error.to_string().contains("exceeds 1024 subcases"));

        let class_name = "A".repeat(MAX_PYTHON_OUTPUT_FIELD_BYTES + 1);
        let amplified = format!("class {class_name}:\n    def test_case(self):\n        pass\n");
        let error = extract_python_test_cases(
            &[PythonSource {
                path: "amplified_test.py",
                content: &amplified,
            }],
            Path::new("."),
        )
        .expect_err("oversized repeated output field must fail in the child");
        assert!(
            error
                .to_string()
                .contains("field symbol exceeds 2048 bytes")
        );

        let mut amplified_total = format!("class {}:\n", "B".repeat(1_800));
        for index in 0..700 {
            amplified_total.push_str(&format!("    def test_{index}(self):\n        pass\n"));
        }
        let error = extract_python_test_cases(
            &[PythonSource {
                path: "amplified_total_test.py",
                content: &amplified_total,
            }],
            Path::new("."),
        )
        .expect_err("cumulative extractor output must stay bounded");
        assert!(error.to_string().contains("exceeds 1048576 output bytes"));

        let parameter_name = "p".repeat(MAX_PYTHON_OUTPUT_FIELD_BYTES + 1);
        let parameter_amplification = format!(
            "import pytest\n@pytest.mark.parametrize(\"{parameter_name}\", [0, 1])\ndef test_parameter_name():\n    pass\n"
        );
        let error = extract_python_test_cases(
            &[PythonSource {
                path: "parameter_amplification_test.py",
                content: &parameter_amplification,
            }],
            Path::new("."),
        )
        .expect_err("parameter names must be bounded before subcase expansion");
        assert!(error.to_string().contains("parameter names exceed"));
    }
}
