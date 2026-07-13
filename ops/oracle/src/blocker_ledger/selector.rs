const MAX_SELECTOR_PARTS: usize = 11;
const MAX_SELECTOR_PART_BYTES: usize = 128;

#[derive(Clone, Copy, Debug)]
pub(crate) struct CargoTestSelector<'a> {
    package: &'a str,
    features: Option<&'a str>,
    target: Option<&'a str>,
    lib: bool,
    filter: Option<&'a str>,
    exact: bool,
}

impl<'a> CargoTestSelector<'a> {
    pub(crate) fn normalize_fixture_argv(argv: &str) -> Result<Option<Vec<String>>, &'static str> {
        let tokens = argv.split('|').collect::<Vec<_>>();
        if tokens.first().copied() != Some("cargo-test") || !tokens.contains(&"--exact") {
            return Ok(None);
        }
        let separator = tokens
            .iter()
            .position(|token| *token == "--")
            .ok_or("exact cargo fixture is missing the libtest separator")?;
        if tokens.get(separator + 1..) != Some(&["--exact"][..]) {
            return Err("exact cargo fixture must end with --|--exact");
        }
        let mut normalized = vec!["cargo".to_string(), "test".to_string()];
        normalized.extend(
            tokens
                .get(1..separator)
                .ok_or("exact cargo fixture has an invalid argument range")?
                .iter()
                .map(|token| (*token).to_string()),
        );
        if normalized.last().map(String::as_str) != Some("--quiet") {
            normalized.push("--quiet".to_string());
        }
        normalized.push("--exact".to_string());
        Ok(Some(normalized))
    }

    pub(crate) fn parse(parts: &'a [String]) -> Result<Self, &'static str> {
        if parts.len() > MAX_SELECTOR_PARTS
            || parts
                .iter()
                .any(|part| part.is_empty() || part.len() > MAX_SELECTOR_PART_BYTES)
        {
            return Err("has too many or oversized parts");
        }

        if let [
            cargo,
            test,
            package_flag,
            package,
            lib,
            filter,
            quiet,
            exact,
        ] = parts
            && cargo == "cargo"
            && test == "test"
            && package_flag == "-p"
            && lib == "--lib"
            && quiet == "--quiet"
            && exact == "--exact"
        {
            validate_package_and_test_name(package, Some(filter))?;
            return Ok(Self {
                package,
                features: None,
                target: None,
                lib: true,
                filter: Some(filter),
                exact: true,
            });
        }

        let (package, features, target, filter, exact) = match parts {
            [
                cargo,
                test,
                package_flag,
                package,
                features_flag,
                features,
                target_flag,
                target,
                filter,
                quiet,
                exact,
            ] if cargo == "cargo"
                && test == "test"
                && package_flag == "-p"
                && features_flag == "--features"
                && target_flag == "--test"
                && quiet == "--quiet"
                && exact == "--exact" =>
            {
                (
                    package.as_str(),
                    Some(features.as_str()),
                    Some(target.as_str()),
                    Some(filter.as_str()),
                    true,
                )
            }
            [
                cargo,
                test,
                package_flag,
                package,
                features_flag,
                features,
                target_flag,
                target,
                filter,
                quiet,
            ] if cargo == "cargo"
                && test == "test"
                && package_flag == "-p"
                && features_flag == "--features"
                && target_flag == "--test"
                && quiet == "--quiet" =>
            {
                (
                    package.as_str(),
                    Some(features.as_str()),
                    Some(target.as_str()),
                    Some(filter.as_str()),
                    false,
                )
            }
            [
                cargo,
                test,
                package_flag,
                package,
                target_flag,
                target,
                filter,
                quiet,
                exact,
            ] if cargo == "cargo"
                && test == "test"
                && package_flag == "-p"
                && target_flag == "--test"
                && quiet == "--quiet"
                && exact == "--exact" =>
            {
                (
                    package.as_str(),
                    None,
                    Some(target.as_str()),
                    Some(filter.as_str()),
                    true,
                )
            }
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
                (
                    package.as_str(),
                    None,
                    Some(target.as_str()),
                    Some(filter.as_str()),
                    false,
                )
            }
            [cargo, test, package_flag, package, filter, quiet, exact]
                if cargo == "cargo"
                    && test == "test"
                    && package_flag == "-p"
                    && quiet == "--quiet"
                    && exact == "--exact" =>
            {
                (package.as_str(), None, None, Some(filter.as_str()), true)
            }
            [cargo, test, package_flag, package, filter, quiet]
                if cargo == "cargo"
                    && test == "test"
                    && package_flag == "-p"
                    && quiet == "--quiet" =>
            {
                (package.as_str(), None, None, Some(filter.as_str()), false)
            }
            [
                cargo,
                test,
                package_flag,
                package,
                features_flag,
                features,
                target_flag,
                target,
                quiet,
            ] if cargo == "cargo"
                && test == "test"
                && package_flag == "-p"
                && features_flag == "--features"
                && target_flag == "--test"
                && quiet == "--quiet" =>
            {
                (
                    package.as_str(),
                    Some(features.as_str()),
                    Some(target.as_str()),
                    None,
                    false,
                )
            }
            [
                cargo,
                test,
                package_flag,
                package,
                target_flag,
                target,
                quiet,
            ] if cargo == "cargo"
                && test == "test"
                && package_flag == "-p"
                && target_flag == "--test"
                && quiet == "--quiet" =>
            {
                (package.as_str(), None, Some(target.as_str()), None, false)
            }
            [cargo, test, package_flag, package, quiet]
                if cargo == "cargo"
                    && test == "test"
                    && package_flag == "-p"
                    && quiet == "--quiet" =>
            {
                (package.as_str(), None, None, None, false)
            }
            _ => return Err("must use the allowlisted cargo test selector shape"),
        };
        if !matches!(
            package,
            "stab-core" | "stab-cli" | "stab-oracle" | "stab-bench"
        ) {
            return Err("uses a package outside the blocker-test allowlist");
        }
        if features.is_some_and(|value| value != "ops-contracts") {
            return Err("uses features outside the selector allowlist");
        }
        if filter.is_some_and(|value| !is_test_name(value))
            || target.is_some_and(|value| !is_test_name(value))
        {
            return Err("contains an invalid test target or filter");
        }
        Ok(Self {
            package,
            features,
            target,
            lib: false,
            filter,
            exact,
        })
    }

    pub(crate) fn is_exact(self) -> bool {
        self.exact && self.filter.is_some()
    }

    pub(crate) fn display(self) -> String {
        let exact = if self.exact { " --exact" } else { "" };
        let features = self
            .features
            .map_or(String::new(), |value| format!(" --features {value}"));
        let filter = self
            .filter
            .map_or(String::new(), |value| format!(" {value}"));
        if self.lib {
            return format!(
                "cargo test -p {}{} --lib --quiet --{}{} --list",
                self.package, features, filter, exact
            );
        }
        match self.target {
            Some(target) => format!(
                "cargo test -p {}{} --test {} --quiet --{}{} --list",
                self.package, features, target, filter, exact
            ),
            None => format!(
                "cargo test -p {}{} --quiet --{}{} --list",
                self.package, features, filter, exact
            ),
        }
    }

    pub(crate) fn args(self) -> Vec<&'a str> {
        let mut args = vec!["test", "-p", self.package];
        if let Some(features) = self.features {
            args.extend(["--features", features]);
        }
        if let Some(target) = self.target {
            args.extend(["--test", target]);
        } else if self.lib {
            args.push("--lib");
        }
        args.extend(["--quiet", "--"]);
        if let Some(filter) = self.filter {
            args.push(filter);
        }
        if self.exact {
            args.push("--exact");
        }
        args.push("--list");
        args
    }

    pub(crate) fn run_args(self) -> Vec<&'a str> {
        let mut args = vec!["test", "-p", self.package];
        if let Some(features) = self.features {
            args.extend(["--features", features]);
        }
        if let Some(target) = self.target {
            args.extend(["--test", target]);
        } else if self.lib {
            args.push("--lib");
        }
        args.extend(["--quiet", "--"]);
        if let Some(filter) = self.filter {
            args.push(filter);
        }
        if self.exact {
            args.push("--exact");
        }
        args
    }
}

pub(crate) fn test_listing_match_count(stdout: &str) -> usize {
    stdout
        .lines()
        .filter(|line| line.ends_with(": test") || line.ends_with(": benchmark"))
        .count()
}

fn is_test_name(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('-')
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-' | b':')
        })
}

fn validate_package_and_test_name(package: &str, filter: Option<&str>) -> Result<(), &'static str> {
    if !matches!(
        package,
        "stab-core" | "stab-cli" | "stab-oracle" | "stab-bench"
    ) {
        return Err("uses a package outside the blocker-test allowlist");
    }
    if filter.is_some_and(|value| !is_test_name(value)) {
        return Err("contains an invalid test target or filter");
    }
    Ok(())
}
