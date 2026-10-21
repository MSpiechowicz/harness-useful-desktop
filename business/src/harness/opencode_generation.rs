//! Which OpenCode the detected binary is: OpenCode 1 (`1.19.3`) and OpenCode 2 (`opencode v2.1.2`)
//! share the `opencode` binary name but differ in config keys, CLI commands, sessions and the chat
//! server. Detection reports the version ([`super::Harness::note_version`]) and the OpenCode
//! adapter picks its behaviour from the generation.

/// The OpenCode generation the app talks to.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum OpenCodeGeneration {
    /// OpenCode 1 (`opencode-ai` on npm, the curl installer), also the guess for an unknown version.
    #[default]
    V1,
    /// OpenCode 2 (`@opencode/cli` on npm).
    V2,
}

impl OpenCodeGeneration {
    /// The generation of `version` (`2.1.2`, `v2.0.0`): major 1 is [`Self::V1`], major 2 and later
    /// [`Self::V2`]. A version without a readable major is [`Self::V1`], the generation the app
    /// supported first.
    pub fn from_version(version: &str) -> Self {
        let digits = version.trim().trim_start_matches(['v', 'V']);
        let major = digits.split('.').next().and_then(|major| major.parse::<u64>().ok());

        match major {
            Some(major) if major >= 2 => Self::V2,
            _ => Self::V1,
        }
    }

    /// A compact code for atomic storage ([`Self::from_code`] reads it back).
    pub const fn code(self) -> u8 {
        match self {
            Self::V1 => 1,
            Self::V2 => 2,
        }
    }

    /// The generation [`Self::code`] returned; any other code is [`Self::V1`].
    pub const fn from_code(code: u8) -> Self {
        match code {
            2 => Self::V2,
            _ => Self::V1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::OpenCodeGeneration;

    #[test]
    fn codes_round_trip_and_unknown_codes_are_v1() {
        for generation in [OpenCodeGeneration::V1, OpenCodeGeneration::V2] {
            assert_eq!(OpenCodeGeneration::from_code(generation.code()), generation);
        }

        assert_eq!(OpenCodeGeneration::from_code(0), OpenCodeGeneration::V1);
    }
}
