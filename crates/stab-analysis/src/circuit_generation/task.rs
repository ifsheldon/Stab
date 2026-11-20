//! Code families and tasks of the circuit generator, with Stim's spellings.

use std::fmt::{self, Display, Formatter};
use std::str::FromStr;

use thiserror::Error;

/// The error-correcting code family a generated circuit implements.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CodeFamily {
    Repetition,
    Surface,
    Color,
}

impl CodeFamily {
    /// Every family, in the order Stim's `gen --help` lists them.
    pub const ALL: [Self; 3] = [Self::Repetition, Self::Surface, Self::Color];

    /// Stim's `--code` spelling, such as `surface_code`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Repetition => "repetition_code",
            Self::Surface => "surface_code",
            Self::Color => "color_code",
        }
    }

    /// Parses one of this family's task names, such as `rotated_memory_x` for the surface code.
    pub fn parse_task(self, name: &str) -> Result<CodeTask, UnknownCodeTask> {
        match self {
            Self::Repetition => name.parse().map(CodeTask::Repetition),
            Self::Surface => name.parse().map(CodeTask::Surface),
            Self::Color => name.parse().map(CodeTask::Color),
        }
    }

    /// Stim's names of this family's tasks.
    pub fn task_names(self) -> &'static [&'static str] {
        match self {
            Self::Repetition => RepetitionCodeTask::NAMES,
            Self::Surface => SurfaceCodeTask::NAMES,
            Self::Color => ColorCodeTask::NAMES,
        }
    }
}

impl Display for CodeFamily {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for CodeFamily {
    type Err = UnknownCodeFamily;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|family| family.as_str() == name)
            .ok_or_else(|| UnknownCodeFamily {
                name: name.to_owned(),
            })
    }
}

/// A `--code` name that is not one of [`CodeFamily::ALL`].
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("unknown code {name:?}; expected repetition_code, surface_code, or color_code")]
pub struct UnknownCodeFamily {
    name: String,
}

impl UnknownCodeFamily {
    /// The rejected name.
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// A task name that the named family does not generate.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("unsupported {family} task {task:?}; expected {}", ExpectedNames(family.task_names()))]
pub struct UnknownCodeTask {
    family: CodeFamily,
    task: String,
}

impl UnknownCodeTask {
    fn new(family: CodeFamily, task: &str) -> Self {
        Self {
            family,
            task: task.to_owned(),
        }
    }

    /// The family whose tasks were searched.
    pub fn family(&self) -> CodeFamily {
        self.family
    }

    /// The rejected task name.
    pub fn task(&self) -> &str {
        &self.task
    }
}

/// Lists names as `a`, `a or b`, or `a, b, or c`.
struct ExpectedNames(&'static [&'static str]);

impl Display for ExpectedNames {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        let count = self.0.len();
        for (index, name) in self.0.iter().enumerate() {
            match (index, count) {
                (0, _) => {}
                (1, 2) => formatter.write_str(" or ")?,
                (index, count) if index + 1 == count => formatter.write_str(", or ")?,
                _ => formatter.write_str(", ")?,
            }
            formatter.write_str(name)?;
        }
        Ok(())
    }
}

/// Defines a family's task enum with Stim's task names, `as_str`, and `FromStr`.
macro_rules! code_tasks {
    ($(#[$meta:meta])* $name:ident, $family:ident, { $($variant:ident => $text:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            /// Every task of this family, in Stim's order.
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            const NAMES: &'static [&'static str] = &[$($text),+];

            /// Stim's `--task` spelling.
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $text),+
                }
            }
        }

        impl Display for $name {
            fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }

        impl FromStr for $name {
            type Err = UnknownCodeTask;

            fn from_str(name: &str) -> Result<Self, Self::Err> {
                Self::ALL
                    .iter()
                    .copied()
                    .find(|task| task.as_str() == name)
                    .ok_or_else(|| UnknownCodeTask::new(CodeFamily::$family, name))
            }
        }

        impl From<$name> for CodeTask {
            fn from(task: $name) -> Self {
                Self::$family(task)
            }
        }
    };
}

code_tasks!(
    /// Repetition-code tasks.
    RepetitionCodeTask, Repetition, {
        Memory => "memory",
    }
);

code_tasks!(
    /// Surface-code tasks.
    SurfaceCodeTask, Surface, {
        RotatedMemoryX => "rotated_memory_x",
        RotatedMemoryZ => "rotated_memory_z",
        UnrotatedMemoryX => "unrotated_memory_x",
        UnrotatedMemoryZ => "unrotated_memory_z",
    }
);

code_tasks!(
    /// Color-code tasks.
    ColorCodeTask, Color, {
        MemoryXyz => "memory_xyz",
    }
);

/// One generator task, which also names its code family.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CodeTask {
    Repetition(RepetitionCodeTask),
    Surface(SurfaceCodeTask),
    Color(ColorCodeTask),
}

impl CodeTask {
    /// The family this task generates.
    pub const fn family(self) -> CodeFamily {
        match self {
            Self::Repetition(_) => CodeFamily::Repetition,
            Self::Surface(_) => CodeFamily::Surface,
            Self::Color(_) => CodeFamily::Color,
        }
    }

    /// Stim's `--task` spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Repetition(task) => task.as_str(),
            Self::Surface(task) => task.as_str(),
            Self::Color(task) => task.as_str(),
        }
    }
}

impl Display for CodeTask {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
