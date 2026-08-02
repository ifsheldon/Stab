use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use crate::{ReleaseError, process, repository, safe_fs};

const CARGO_CONFIG: &[u8] = br#"[registry]
default = "crates-io"
credential-provider = "cargo:token"
global-credential-providers = ["cargo:token"]

[registries.crates-io]
protocol = "sparse"

[net]
git-fetch-with-cli = false
"#;
const ISOLATED_CARGO_COMMAND: &str = "__isolated-cargo";
const SYSTEM_PATH: &str = "/usr/bin:/bin";

pub(crate) struct CargoSandbox {
    launcher: safe_fs::DescriptorProgram,
    cargo: safe_fs::DescriptorProgram,
    rustc: safe_fs::DescriptorProgram,
    rustdoc: safe_fs::DescriptorProgram,
    home: safe_fs::RetainedDirectory,
    cargo_home: safe_fs::RetainedDirectory,
    temporary: safe_fs::RetainedDirectory,
    target: PathBuf,
    manifest: PathBuf,
    config: PathBuf,
    config_identity: safe_fs::FileIdentity,
}

impl CargoSandbox {
    pub(crate) fn create(
        root: &Path,
        work: &safe_fs::RetainedDirectory,
        target: &safe_fs::RetainedDirectory,
    ) -> Result<Self, ReleaseError> {
        let home = private_directory(work, OsStr::new("home"))?;
        let cargo_home = private_directory(work, OsStr::new("cargo-home"))?;
        let temporary = private_directory(work, OsStr::new("tmp"))?;
        let config = cargo_home.path().join("config.toml");
        let config_file = cargo_home.write_new(OsStr::new("config.toml"), CARGO_CONFIG)?;
        set_private_file_permissions(&config_file, &config)?;
        let config_identity = safe_fs::file_identity(&config_file, &config)?;
        cargo_home.sync()?;

        Ok(Self {
            launcher: retain_program(&std::env::current_exe().map_err(|source| {
                ReleaseError::CommandIo {
                    program: "current release executable".to_string(),
                    source,
                }
            })?)?,
            cargo: retain_program(&resolve_toolchain_program(root, "cargo")?)?,
            rustc: retain_program(&resolve_toolchain_program(root, "rustc")?)?,
            rustdoc: retain_program(&resolve_toolchain_program(root, "rustdoc")?)?,
            home,
            cargo_home,
            temporary,
            target: target.path().to_path_buf(),
            manifest: root.join("Cargo.toml"),
            config,
            config_identity,
        })
    }

    pub(crate) fn run<I, S>(
        &self,
        root: &Path,
        args: I,
        timeout: Duration,
        output_limit: usize,
    ) -> Result<process::ProcessOutput, ReleaseError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.run_inner(root, args, timeout, output_limit)
    }

    pub(crate) fn run_capture<I, S>(
        &self,
        root: &Path,
        args: I,
        timeout: Duration,
        output_limit: usize,
    ) -> Result<String, ReleaseError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let output = self.run(root, args, timeout, output_limit)?;
        String::from_utf8(output.stdout).map_err(ReleaseError::from)
    }

    fn run_inner<I, S>(
        &self,
        root: &Path,
        args: I,
        timeout: Duration,
        output_limit: usize,
    ) -> Result<process::ProcessOutput, ReleaseError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.revalidate()?;
        let mut helper_args = self.helper_arguments();
        helper_args.push(OsString::from("--"));
        helper_args.extend(arguments_with_manifest(args, &self.manifest));
        let result = process::run(
            root,
            self.launcher.path().as_os_str(),
            &helper_args,
            &[],
            timeout,
            output_limit,
        );
        self.revalidate()?;
        result
    }

    fn helper_arguments(&self) -> Vec<OsString> {
        vec![
            OsString::from(ISOLATED_CARGO_COMMAND),
            OsString::from("--cargo"),
            self.cargo.path().as_os_str().to_os_string(),
            OsString::from("--rustc"),
            self.rustc.path().as_os_str().to_os_string(),
            OsString::from("--rustdoc"),
            self.rustdoc.path().as_os_str().to_os_string(),
            OsString::from("--home"),
            self.home.path().as_os_str().to_os_string(),
            OsString::from("--cargo-home"),
            self.cargo_home.path().as_os_str().to_os_string(),
            OsString::from("--target"),
            self.target.as_os_str().to_os_string(),
            OsString::from("--temporary"),
            self.temporary.path().as_os_str().to_os_string(),
            OsString::from("--config"),
            self.config.as_os_str().to_os_string(),
        ]
    }

    fn revalidate(&self) -> Result<(), ReleaseError> {
        self.home.revalidate()?;
        self.cargo_home.revalidate()?;
        self.temporary.revalidate()?;
        safe_fs::require_same_path_identity(&self.config, self.config_identity)
    }
}

fn arguments_with_manifest<I, S>(args: I, manifest: &Path) -> Vec<OsString>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut args = args
        .into_iter()
        .map(|arg| arg.as_ref().to_os_string())
        .collect::<Vec<_>>();
    let insertion = args
        .iter()
        .position(|arg| arg == "--")
        .unwrap_or(args.len());
    args.splice(
        insertion..insertion,
        [
            OsString::from("--manifest-path"),
            manifest.as_os_str().to_os_string(),
        ],
    );
    args
}

#[doc(hidden)]
#[allow(
    clippy::too_many_arguments,
    reason = "hidden exec boundary mirrors its typed CLI fields"
)]
pub(crate) fn execute_isolated_cargo(
    cargo: &Path,
    rustc: &Path,
    rustdoc: &Path,
    home: &Path,
    cargo_home: &Path,
    target: &Path,
    temporary: &Path,
    config: &Path,
    cargo_args: &[OsString],
) -> Result<(), ReleaseError> {
    require_no_root_cargo_config()?;
    let request = CargoEnvironment {
        rustc,
        rustdoc,
        home,
        cargo_home,
        target,
        temporary,
    };
    let mut command = Command::new(cargo);
    configure_command(&mut command, &request);
    command.arg("--config").arg(config).args(cargo_args);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;

        let source = command.exec();
        Err(ReleaseError::CommandIo {
            program: cargo.to_string_lossy().into_owned(),
            source,
        })
    }
    #[cfg(not(unix))]
    {
        let _ = command;
        Err(ReleaseError::UnsupportedPlatform)
    }
}

struct CargoEnvironment<'a> {
    rustc: &'a Path,
    rustdoc: &'a Path,
    home: &'a Path,
    cargo_home: &'a Path,
    target: &'a Path,
    temporary: &'a Path,
}

fn configure_command(command: &mut Command, request: &CargoEnvironment<'_>) {
    command
        .current_dir(Path::new("/"))
        .env_clear()
        .env("HOME", request.home)
        .env("CARGO_HOME", request.cargo_home)
        .env("CARGO_TARGET_DIR", request.target)
        .env("CARGO_TERM_COLOR", "never")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("LANG", "C")
        .env("LC_ALL", "C")
        .env("PATH", SYSTEM_PATH)
        .env("RUSTC", request.rustc)
        .env("RUSTDOC", request.rustdoc)
        .env("TMPDIR", request.temporary);
}

fn private_directory(
    work: &safe_fs::RetainedDirectory,
    name: &OsStr,
) -> Result<safe_fs::RetainedDirectory, ReleaseError> {
    let directory = work.create_directory(name)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;

        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700))
            .map_err(|source| ReleaseError::io(directory.path(), source))?;
        directory.revalidate()?;
        Ok(directory)
    }
    #[cfg(not(unix))]
    {
        let _ = directory;
        Err(ReleaseError::UnsupportedPlatform)
    }
}

fn set_private_file_permissions(file: &fs::File, path: &Path) -> Result<(), ReleaseError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;

        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|source| ReleaseError::io(path, source))?;
        file.sync_all()
            .map_err(|source| ReleaseError::io(path, source))?;
        Ok(())
    }
    #[cfg(not(unix))]
    {
        let _ = (file, path);
        Err(ReleaseError::UnsupportedPlatform)
    }
}

fn resolve_toolchain_program(root: &Path, program: &str) -> Result<PathBuf, ReleaseError> {
    repository::toolchain_program(root, program)
}

fn retain_program(path: &Path) -> Result<safe_fs::DescriptorProgram, ReleaseError> {
    let file = safe_fs::open_regular_file(path)?;
    safe_fs::descriptor_program(&file, path)
}

fn require_no_root_cargo_config() -> Result<(), ReleaseError> {
    for path in [
        Path::new("/.cargo/config"),
        Path::new("/.cargo/config.toml"),
    ] {
        match fs::symlink_metadata(path) {
            Ok(_) => {
                return Err(ReleaseError::PackageContract(format!(
                    "host root Cargo configuration {} would violate the private release configuration boundary",
                    path.display()
                )));
            }
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {}
            Err(source) => return Err(ReleaseError::io(path, source)),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::process::Stdio;

    use super::*;

    struct EnvironmentFixture {
        home: PathBuf,
        cargo_home: PathBuf,
        target: PathBuf,
        temporary: PathBuf,
        config: PathBuf,
    }

    impl EnvironmentFixture {
        fn new(root: &Path) -> Self {
            let cargo_home = root.join("private-cargo-home");
            Self {
                home: root.join("private-home"),
                config: cargo_home.join("config.toml"),
                cargo_home,
                target: root.join("private-target"),
                temporary: root.join("private-tmp"),
            }
        }

        fn request(&self) -> CargoEnvironment<'_> {
            CargoEnvironment {
                rustc: Path::new("/toolchain/rustc"),
                rustdoc: Path::new("/toolchain/rustdoc"),
                home: &self.home,
                cargo_home: &self.cargo_home,
                target: &self.target,
                temporary: &self.temporary,
            }
        }
    }

    fn run_probe(test_name: &str) {
        let root = tempfile::tempdir().expect("private Cargo root");
        let fixture = EnvironmentFixture::new(root.path());
        let request = fixture.request();
        prepare_fixture(&fixture);

        let mut command = Command::new(std::env::current_exe().expect("test executable"));
        command
            .args(["--ignored", "--exact", test_name, "--nocapture"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env("HOME", "/attacker/home")
            .env("CARGO_HOME", "/attacker/cargo-home")
            .env("CARGO_REGISTRY_TOKEN", "ambient-secret")
            .env("CARGO_REGISTRIES_CRATES_IO_TOKEN", "ambient-other-secret")
            .env("CARGO_TARGET_DIR", "/attacker/target")
            .env("RUSTC_WRAPPER", "/attacker/wrapper")
            .env("STAB_RELEASE_HOSTILE_SECRET", "hostile-secret");
        configure_command(&mut command, &request);
        let output = command.output().expect("run isolation probe");
        assert!(
            output.status.success(),
            "stdout={}\nstderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn prepare_fixture(fixture: &EnvironmentFixture) {
        fs::create_dir_all(&fixture.cargo_home).expect("Cargo home");
        fs::write(&fixture.config, CARGO_CONFIG).expect("private config");
        fs::create_dir_all(&fixture.home).expect("private home");
        fs::create_dir_all(&fixture.target).expect("private target");
        fs::create_dir_all(&fixture.temporary).expect("private temporary directory");
    }

    fn assert_probe_environment() {
        let keys = std::env::vars_os()
            .map(|(key, _)| key.to_string_lossy().into_owned())
            .collect::<BTreeSet<_>>();
        let expected = [
            "CARGO_HOME",
            "CARGO_TARGET_DIR",
            "CARGO_TERM_COLOR",
            "GIT_CONFIG_GLOBAL",
            "GIT_CONFIG_NOSYSTEM",
            "GIT_TERMINAL_PROMPT",
            "HOME",
            "LANG",
            "LC_ALL",
            "PATH",
            "RUSTC",
            "RUSTDOC",
            "TMPDIR",
        ]
        .map(str::to_string)
        .into_iter()
        .collect::<BTreeSet<_>>();
        assert_eq!(keys, expected);
        let cargo_home = PathBuf::from(std::env::var_os("CARGO_HOME").expect("Cargo home"));
        assert_eq!(
            fs::read(cargo_home.join("config.toml")).expect("private config"),
            CARGO_CONFIG
        );
    }

    #[test]
    fn hostile_ambient_cargo_state_and_secrets_are_removed() {
        run_probe("cargo::tests::probe_without_registry_token");
    }

    #[test]
    fn private_config_explicitly_selects_cargo_token() {
        let config = std::str::from_utf8(CARGO_CONFIG).expect("UTF-8 config");
        assert!(config.contains("credential-provider = \"cargo:token\""));
        assert!(config.contains("global-credential-providers = [\"cargo:token\"]"));
        assert!(!config.contains("token ="));
    }

    #[cfg(unix)]
    #[test]
    fn private_cargo_paths_are_owner_only() {
        use std::os::unix::fs::PermissionsExt as _;

        let root = tempfile::tempdir().expect("private path root");
        let work =
            safe_fs::RetainedDirectory::create_new_under(root.path(), Path::new("work"), None)
                .expect("retained work directory");
        let cargo_home = private_directory(&work, OsStr::new("cargo-home")).expect("Cargo home");
        let config_path = cargo_home.path().join("config.toml");
        let config = cargo_home
            .write_new(OsStr::new("config.toml"), CARGO_CONFIG)
            .expect("private config");
        set_private_file_permissions(&config, &config_path).expect("private permissions");

        assert_eq!(
            fs::metadata(cargo_home.path())
                .expect("Cargo home metadata")
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(
            config
                .metadata()
                .expect("Cargo config metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }

    #[test]
    fn workspace_manifest_precedes_subcommand_arguments() {
        assert_eq!(
            arguments_with_manifest(
                [
                    "run",
                    "--quiet",
                    "--package",
                    "stab-architecture",
                    "--",
                    "check"
                ],
                Path::new("/source/Cargo.toml"),
            ),
            [
                "run",
                "--quiet",
                "--package",
                "stab-architecture",
                "--manifest-path",
                "/source/Cargo.toml",
                "--",
                "check",
            ]
            .map(OsString::from)
        );
    }

    #[test]
    fn real_cargo_ignores_hostile_ambient_and_manifest_parent_config() {
        let root = tempfile::tempdir().expect("Cargo isolation fixture");
        let fixture = EnvironmentFixture::new(root.path());
        prepare_fixture(&fixture);
        let hostile_config = root.path().join(".cargo");
        fs::create_dir_all(&hostile_config).expect("hostile config directory");
        fs::write(
            hostile_config.join("config.toml"),
            "[build]\nrustc-wrapper = \"/attacker/missing-wrapper\"\n[env]\nSTAB_RELEASE_HOSTILE_SECRET = \"from-config\"\n",
        )
        .expect("hostile config");
        let project = root.path().join("project");
        fs::create_dir_all(project.join("src")).expect("project source");
        fs::write(
            project.join("Cargo.toml"),
            "[package]\nname = \"cargo-isolation-probe\"\nversion = \"0.0.0\"\nedition = \"2024\"\nbuild = \"build.rs\"\n[workspace]\n",
        )
        .expect("probe manifest");
        fs::write(project.join("src/lib.rs"), "pub fn probe() {}\n").expect("probe source");
        fs::write(
            project.join("build.rs"),
            r#"fn main() {
    for key in [
        "CARGO_REGISTRY_TOKEN",
        "CARGO_REGISTRIES_CRATES_IO_TOKEN",
        "RUSTC_WRAPPER",
        "STAB_RELEASE_HOSTILE_SECRET",
    ] {
        assert!(std::env::var_os(key).is_none(), "forbidden variable reached build.rs: {key}");
    }
}
"#,
        )
        .expect("probe build script");

        let repository_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let cargo = resolve_toolchain_program(&repository_root, "cargo").expect("Cargo program");
        let rustc = resolve_toolchain_program(&repository_root, "rustc").expect("rustc program");
        let rustdoc =
            resolve_toolchain_program(&repository_root, "rustdoc").expect("rustdoc program");
        let request = CargoEnvironment {
            rustc: &rustc,
            rustdoc: &rustdoc,
            home: &fixture.home,
            cargo_home: &fixture.cargo_home,
            target: &fixture.target,
            temporary: &fixture.temporary,
        };
        let mut command = Command::new(cargo);
        command
            .env("HOME", root.path())
            .env("CARGO_HOME", &hostile_config)
            .env("CARGO_REGISTRY_TOKEN", "ambient-secret")
            .env("CARGO_REGISTRIES_CRATES_IO_TOKEN", "ambient-other-secret")
            .env("RUSTC_WRAPPER", "/attacker/ambient-wrapper")
            .env("STAB_RELEASE_HOSTILE_SECRET", "from-environment");
        configure_command(&mut command, &request);
        let output = command
            .arg("--config")
            .arg(&fixture.config)
            .args(["check", "--quiet", "--manifest-path"])
            .arg(project.join("Cargo.toml"))
            .output()
            .expect("run isolated Cargo");
        assert!(
            output.status.success(),
            "stdout={}\nstderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    #[ignore = "executed only as a subprocess by Cargo isolation tests"]
    fn probe_without_registry_token() {
        assert_probe_environment();
    }
}
