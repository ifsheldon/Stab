const MAX_SELECTOR_PARTS: usize = 8;
const MAX_SELECTOR_PART_BYTES: usize = 128;

#[derive(Clone, Copy, Debug)]
pub(super) struct CargoTestSelector<'a> {
    package: &'a str,
    target: Option<&'a str>,
    filter: &'a str,
}

impl<'a> CargoTestSelector<'a> {
    pub(super) fn parse(parts: &'a [String]) -> Result<Self, &'static str> {
        if parts.len() > MAX_SELECTOR_PARTS
            || parts
                .iter()
                .any(|part| part.is_empty() || part.len() > MAX_SELECTOR_PART_BYTES)
        {
            return Err("has too many or oversized parts");
        }

        let (package, target, filter) = match parts {
            [
                cargo,
                test,
                package_flag,
                package,
                target_flag,
                target,
                filter,
                quiet,
            ] if cargo == "cargo"
                && test == "test"
                && package_flag == "-p"
                && target_flag == "--test"
                && quiet == "--quiet" =>
            {
                (package.as_str(), Some(target.as_str()), filter.as_str())
            }
            [cargo, test, package_flag, package, filter, quiet]
                if cargo == "cargo"
                    && test == "test"
                    && package_flag == "-p"
                    && quiet == "--quiet" =>
            {
                (package.as_str(), None, filter.as_str())
            }
            _ => return Err("must use the allowlisted cargo test selector shape"),
        };
        if !matches!(
            package,
            "stab-core" | "stab-cli" | "stab-oracle" | "stab-bench"
        ) {
            return Err("uses a package outside the blocker-test allowlist");
        }
        if !is_test_name(filter) || target.is_some_and(|value| !is_test_name(value)) {
            return Err("contains an invalid test target or filter");
        }
        Ok(Self {
            package,
            target,
            filter,
        })
    }

    pub(super) fn display(self) -> String {
        match self.target {
            Some(target) => format!(
                "cargo test -p {} --test {} --quiet -- {} --list",
                self.package, target, self.filter
            ),
            None => format!(
                "cargo test -p {} --quiet -- {} --list",
                self.package, self.filter
            ),
        }
    }

    pub(super) fn args(self) -> Vec<&'a str> {
        let mut args = vec!["test", "-p", self.package];
        if let Some(target) = self.target {
            args.extend(["--test", target]);
        }
        args.extend(["--quiet", "--", self.filter, "--list"]);
        args
    }
}

pub(super) fn test_listing_has_match(stdout: &str) -> bool {
    stdout
        .lines()
        .any(|line| line.ends_with(": test") || line.ends_with(": benchmark"))
}

fn is_test_name(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('-')
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-' | b':')
        })
}
