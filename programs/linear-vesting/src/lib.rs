use anchor_lang::{prelude::*, solana_program::clock};
use anchor_spl::token::{self, Mint, SetAuthority, TokenAccount, Transfer};
use spl_token::instruction::AuthorityType;

pub mod error;
use crate::{error::LinearVestingError};

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

const VAULT_PDA_SEED: &[u8] = b"token-vault";
const VAULT_AUTHORITY_PDA_SEED: &[u8] = b"vault-authority";


#[program]
pub mod linear_vesting {
    use super::*;


    pub fn initialize(
        ctx: Context<Initialize>,
        amount: u64,
        start_ts: i64,
        cliff_ts: i64,
        duration: i64,
        revocable: bool,
    ) -> ProgramResult {
        ctx.accounts.vesting_account.start_ts = start_ts;
        ctx.accounts.vesting_account.cliff_ts = cliff_ts;
        ctx.accounts.vesting_account.duration = duration;
        ctx.accounts.vesting_account.revocable = revocable;
        ctx.accounts.vesting_account.revoked = false;

        ctx.accounts.vesting_account.beneficiary = *ctx.accounts.beneficiary.key;
        ctx.accounts.vesting_account.owner = *ctx.accounts.owner.key;
        ctx.accounts.vesting_account.mint = *ctx.accounts.mint.to_account_info().key;

        ctx.accounts.vesting_account.total_deposited_amount = amount;
        ctx.accounts.vesting_account.released_amount = 0;


        let (vault_authority, _vault_authority_bump) =
            Pubkey::find_program_address(&[VAULT_AUTHORITY_PDA_SEED], ctx.program_id);

        token::set_authority(
            ctx.accounts.into_set_authority_context(),
            AuthorityType::AccountOwner,
            Some(vault_authority),
        )?;

        token::transfer(
            ctx.accounts.into_transfer_to_pda_context(),
            ctx.accounts.vesting_account.total_deposited_amount,
        )?;

        Ok(())
    }

    pub fn withdraw(
        ctx: Context<Withdraw>
    ) -> ProgramResult {
        if !ctx.accounts.beneficiary.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }

        let current_time = clock::Clock::get().unwrap().unix_timestamp;
        let start_ts = ctx.accounts.vesting_account.start_ts;
        let cliff_ts = ctx.accounts.vesting_account.cliff_ts;
        let duration = ctx.accounts.vesting_account.duration;
        let revoked = ctx.accounts.vesting_account.revoked;
        let released_amount = ctx.accounts.vesting_account.released_amount;
        let current_balance = ctx.accounts.vault_account.amount;
        let total_balance = released_amount + current_balance;

        let mut unreleased_amount = 0;

        if current_time < start_ts + cliff_ts {
            unreleased_amount = 0;
        } else if current_time >= start_ts + duration || revoked {
            unreleased_amount = current_balance;
        } else {
            unreleased_amount = ((total_balance * (current_time - start_ts) as u64) / duration as u64) - released_amount;
        }

        if unreleased_amount <= 0 {
            return Err(LinearVestingError::NoTokensDue.into());
        }

        ctx.accounts.vesting_account.released_amount += unreleased_amount;

        let (_vault_authority, vault_authority_bump) =
            Pubkey::find_program_address(&[VAULT_AUTHORITY_PDA_SEED], ctx.program_id);
        let authority_seeds = &[&VAULT_AUTHORITY_PDA_SEED[..], &[vault_authority_bump]];
        
        token::transfer(
            ctx.accounts.into_transfer_to_beneficiary_context().with_signer(&[&authority_seeds[..]]),
            unreleased_amount,
        )?;

        Ok(())
    }

    pub fn revoke(
        ctx: Context<Revoke>
    ) -> ProgramResult {
        if !ctx.accounts.owner.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }
        if ctx.accounts.owner.key != &ctx.accounts.vesting_account.owner {
            return Err(LinearVestingError::NotMatchingOwner.into());
        }
        if !ctx.accounts.vesting_account.revocable {
            return Err(LinearVestingError::NotRevocable.into());
        }
        if ctx.accounts.vesting_account.revoked {
            return Err(LinearVestingError::TokenAlreadyRevoked.into());
        }

        let current_time = clock::Clock::get().unwrap().unix_timestamp;
        let start_ts = ctx.accounts.vesting_account.start_ts;
        let cliff_ts = ctx.accounts.vesting_account.cliff_ts;
        let duration = ctx.accounts.vesting_account.duration;
        let revoked = ctx.accounts.vesting_account.revoked;
        let released_amount = ctx.accounts.vesting_account.released_amount;
        let current_balance = ctx.accounts.vault_account.amount;
        let total_balance = current_balance + released_amount;
        
        let mut unreleased_amount = 0;

        if current_time < start_ts + cliff_ts {
            unreleased_amount = 0;
        } else if current_time >= start_ts + duration || revoked {
            unreleased_amount = current_balance;
        } else {
            unreleased_amount = ((total_balance * (current_time - start_ts) as u64) / duration as u64) - released_amount;
        }

        ctx.accounts.vesting_account.revoked = true;

        let (_vault_authority, vault_authority_bump) =
            Pubkey::find_program_address(&[VAULT_AUTHORITY_PDA_SEED], ctx.program_id);
        let authority_seeds = &[&VAULT_AUTHORITY_PDA_SEED[..], &[vault_authority_bump]];
        
        token::transfer(
            ctx.accounts.into_transfer_to_owner_context().with_signer(&[&authority_seeds[..]]),
            current_balance - unreleased_amount,
        )?;

        Ok(())
    }

}

#[derive(Accounts)]
#[instruction(amount: u64,
  start_ts: i64,
  cliff_ts: i64,
  duration: i64,
  revocable: bool)]
pub struct Initialize<'info> {
    #[account(mut, signer)]
    pub owner: AccountInfo<'info>,
    pub beneficiary: AccountInfo<'info>,
    pub mint: Account<'info, Mint>,
    #[account(mut)]
    pub beneficiary_ata: Account<'info, TokenAccount>,
    #[account(
        init,
        seeds = [VAULT_PDA_SEED, &beneficiary_ata.to_account_info().key.to_bytes()], bump,
        payer = owner,
        token::mint = mint,
        token::authority = owner,
    )]
    pub vault_account: Account<'info, TokenAccount>,
    #[account(
        mut,
        constraint = owner_token_account.amount >= amount
    )]
    pub owner_token_account: Account<'info, TokenAccount>,
    #[account(
        init,
        seeds = [&beneficiary_ata.to_account_info().key.to_bytes()],
        bump,
        payer = owner,
        space = 8 * 19
    )]
    pub vesting_account: Account<'info, VestingAccount>,
    pub system_program: AccountInfo<'info>,
    pub rent: Sysvar<'info, Rent>,
    pub token_program: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct Withdraw<'info> {
    #[account(mut, signer)]
    pub beneficiary: AccountInfo<'info>,
    #[account(mut)]
    pub beneficiary_ata: Account<'info, TokenAccount>,
    #[account(mut)]
    pub vault_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub vesting_account: Account<'info, VestingAccount>,
    pub vault_authority: AccountInfo<'info>,
    pub system_program: AccountInfo<'info>,
    pub rent: Sysvar<'info, Rent>,
    pub token_program: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct Revoke<'info> {
    #[account(mut, signer)]
    pub owner: AccountInfo<'info>,
    #[account(mut)]
    pub vault_account: Account<'info, TokenAccount>,
    #[account(mut)]
    pub vesting_account: Account<'info, VestingAccount>,
    #[account(mut)]
    pub owner_token_account: Account<'info, TokenAccount>,
    pub vault_authority: AccountInfo<'info>,
    pub system_program: AccountInfo<'info>,
    pub rent: Sysvar<'info, Rent>,
    pub token_program: AccountInfo<'info>,
}

#[account]
pub struct VestingAccount {
    /// The investor who will received vested tokens
    pub beneficiary: Pubkey,
    /// The timestamp for when the lock ends and vesting begins
    pub start_ts: i64,
    /// The timestamp for when the cliff ends (vesting happens during cliff!)
    pub cliff_ts: i64,
    /// The duration of the vesting period
    pub duration: i64,
    /// Whether this vesting account is revocable
    pub revocable: bool,
    /// Owner that can revoke the account
    pub owner: Pubkey,
    /// The mint of the SPL token locked up.
    pub mint: Pubkey,
    /// Total amount to be vested
    pub total_deposited_amount: u64,
    /// Amount that has been released
    pub released_amount: u64,
    /// The account is revoked
    pub revoked: bool
}

impl<'info> Initialize<'info> {
    fn into_transfer_to_pda_context(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self.owner_token_account.to_account_info().clone(),
            to: self.vault_account.to_account_info().clone(),
            authority: self.owner.clone(),
        };
        CpiContext::new(self.token_program.clone(), cpi_accounts)
    }

    fn into_set_authority_context(&self) -> CpiContext<'_, '_, '_, 'info, SetAuthority<'info>> {
        let cpi_accounts = SetAuthority {
            account_or_mint: self.vault_account.to_account_info().clone(),
            current_authority: self.owner.clone(),
        };
        CpiContext::new(self.token_program.clone(), cpi_accounts)
    }
}

impl<'info> Withdraw<'info> {
    fn into_transfer_to_beneficiary_context(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self.vault_account.to_account_info().clone(),
            to: self.beneficiary_ata.to_account_info().clone(),
            authority: self.vault_authority.clone(),
        };
        CpiContext::new(self.token_program.clone(), cpi_accounts)
    }
}

impl<'info> Revoke<'info> {
    fn into_transfer_to_owner_context(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self.vault_account.to_account_info().clone(),
            to: self.owner_token_account.to_account_info().clone(),
            authority: self.vault_authority.clone(),
        };
        CpiContext::new(self.token_program.clone(), cpi_accounts)
    }
}
