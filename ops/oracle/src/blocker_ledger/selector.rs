const MAX_SELECTOR_PARTS: usize = 9;
const MAX_SELECTOR_PART_BYTES: usize = 128;

#[derive(Clone, Copy, Debug)]
pub(crate) struct CargoTestSelector<'a> {
    package: &'a str,
    target: Option<&'a str>,
    filter: &'a str,
    exact: bool,
}

impl<'a> CargoTestSelector<'a> {
    pub(crate) fn parse(parts: &'a [String]) -> Result<Self, &'static str> {
        if parts.len() > MAX_SELECTOR_PARTS
            || parts
                .iter()
                .any(|part| part.is_empty() || part.len() > MAX_SELECTOR_PART_BYTES)
        {
            return Err("has too many or oversized parts");
        }

        let (package, target, filter, exact) = match parts {
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
                    Some(target.as_str()),
                    filter.as_str(),
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
                    Some(target.as_str()),
                    filter.as_str(),
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
                (package.as_str(), None, filter.as_str(), true)
            }
            [cargo, test, package_flag, package, filter, quiet]
                if cargo == "cargo"
                    && test == "test"
                    && package_flag == "-p"
                    && quiet == "--quiet" =>
            {
                (package.as_str(), None, filter.as_str(), false)
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
            exact,
        })
    }

    pub(crate) fn is_exact(self) -> bool {
        self.exact
    }

    pub(crate) fn display(self) -> String {
        let exact = if self.exact { " --exact" } else { "" };
        match self.target {
            Some(target) => format!(
                "cargo test -p {} --test {} --quiet -- {}{} --list",
                self.package, target, self.filter, exact
            ),
            None => format!(
                "cargo test -p {} --quiet -- {}{} --list",
                self.package, self.filter, exact
            ),
        }
    }

    pub(crate) fn args(self) -> Vec<&'a str> {
        let mut args = vec!["test", "-p", self.package];
        if let Some(target) = self.target {
            args.extend(["--test", target]);
        }
        args.extend(["--quiet", "--", self.filter]);
        if self.exact {
            args.push("--exact");
        }
        args.push("--list");
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
