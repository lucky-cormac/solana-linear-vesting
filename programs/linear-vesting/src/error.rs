use solana_program::{decode_error::DecodeError, program_error::ProgramError};
use thiserror::Error;
use num_derive::FromPrimitive;

#[derive(Error, FromPrimitive, Debug, Copy, Clone)]
pub enum LinearVestingError {
    #[error("TokenVesting: beneficiary is the zero address")]
    BeneficiaryZeroAddress,
    #[error("TokenVesting: cliff is longer than duration")]
    CliffLongerThanDuration,
    #[error("TokenVesting: duration is 0")]
    ZeroDuration,
    #[error("TokenVesting: final time is before current time")]
    FinalTimeBeforeCurrentTime,
    #[error("TokenVesting: no tokens are due")]
    NoTokensDue,
    #[error("TokenVesting: not matching owner")]
    NotMatchingOwner,
    #[error("TokenVesting: cannot revoke")]
    NotRevocable,
    #[error("TokenVesting: token already revoked")]
    TokenAlreadyRevoked,
}

impl<T> DecodeError<T> for LinearVestingError {
    fn type_of() -> &'static str {
        "Error"
    }
}

impl From<LinearVestingError> for ProgramError {
    fn from(err: LinearVestingError) -> Self {
        ProgramError::Custom(err as u32)
    }
}
