use anchor_lang::prelude::*;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};

declare_id!("BxUWyfg1xLo4i2rKXXjYUk5e2G5YHcbLXX97FaZGcrhL");

#[program]
pub mod gm_anchor {
    use super::*;

    pub fn initialize_gm_account(ctx: Context<InitializeGmAccount>) -> Result<()> {
        let gm_account = &mut ctx.accounts.gm_account;
        gm_account.name = String::new();
        gm_account.first_greeting_time = 0;
        gm_account.staked_amount = [0; 4];
        gm_account.last_stake_time = [0; 4];
        Ok(())
    }

    pub fn initialize_staking_wallet(ctx: Context<InitializeStakingWallet>) -> Result<()> {
        msg!("Initializing staking wallet");
        Ok(())
    }

    pub fn create_user_ata(ctx: Context<CreateUserATA>) -> Result<()> {
        msg!("Associated Token Account created for the user");
        Ok(())
    }

    pub fn stake_tokens(ctx: Context<StakeTokens>, amount: u64, epoch: u8) -> Result<()> {
        require!(epoch >= 1 && epoch <= 4, ErrorCode::InvalidEpoch);

        let gm_account = &mut ctx.accounts.gm_account;
        let user = &ctx.accounts.user;
        let user_token_account = &ctx.accounts.user_token_account;
        let staking_wallet = &ctx.accounts.staking_wallet;

        require_keys_eq!(
            ctx.accounts.user_token_account.mint,
            ctx.accounts.staking_wallet.mint,
            ErrorCode::InvalidMint
        );

        let cpi_accounts = Transfer {
            from: user_token_account.to_account_info(),
            to: staking_wallet.to_account_info(),
            authority: user.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
        token::transfer(cpi_ctx, amount)?;

        let epoch_index = (epoch - 1) as usize;
        gm_account.staked_amount[epoch_index] = gm_account.staked_amount[epoch_index]
            .checked_add(amount)
            .unwrap();
        gm_account.last_stake_time[epoch_index] = ctx.accounts.clock.unix_timestamp;

        msg!(
            "Staked {} tokens for epoch {} for {}",
            amount,
            epoch,
            gm_account.name
        );
        Ok(())
    }

    pub fn unstake_tokens(ctx: Context<UnstakeTokens>, epoch: u8) -> Result<()> {
        require!(epoch >= 1 && epoch <= 4, ErrorCode::InvalidEpoch);

        let epoch_index = (epoch - 1) as usize;
        let gm_account = &mut ctx.accounts.gm_account;
        let amount = gm_account.staked_amount[epoch_index];
        require!(amount > 0, ErrorCode::InsufficientStakedAmount);

        let current_time = ctx.accounts.clock.unix_timestamp;
        let stake_time = gm_account.last_stake_time[epoch_index];
        let time_staked = current_time - stake_time;

        let epoch_duration = match epoch {
            1 => 60,  // 1 minute
            2 => 120, // 2 minutes
            3 => 180, // 3 minutes
            4 => 240, // 4 minutes
            _ => return Err(ErrorCode::InvalidEpoch.into()),
        };

        require!(
            time_staked >= epoch_duration,
            ErrorCode::StakingPeriodNotEnded
        );

        let apr = match epoch {
            1 => 30, // 30% APR
            2 => 40, // 40% APR
            3 => 50, // 50% APR
            4 => 60, // 60% APR
            _ => return Err(ErrorCode::InvalidEpoch.into()),
        };

        let reward = (amount as u128)
            .checked_mul(apr as u128)
            .unwrap()
            .checked_mul(time_staked as u128)
            .unwrap()
            .checked_div(365 * 24 * 60 * 60 * 100)
            .unwrap() as u64;

        let total_amount = amount.checked_add(reward).unwrap();

        let seeds = &[b"staking_wallet".as_ref(), &[ctx.bumps.staking_wallet]];
        let signer = &[&seeds[..]];

        let cpi_accounts = Transfer {
            from: ctx.accounts.staking_wallet.to_account_info(),
            to: ctx.accounts.user_token_account.to_account_info(),
            authority: ctx.accounts.staking_wallet.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer);

        token::transfer(cpi_ctx, total_amount)?;

        gm_account.staked_amount[epoch_index] = 0;
        gm_account.last_stake_time[epoch_index] = 0;

        msg!(
            "Unstaked {} tokens with {} reward for epoch {} for {}",
            amount,
            reward,
            epoch,
            gm_account.name
        );
        Ok(())
    }

    pub fn get_staked_amount(ctx: Context<GetStakedAmount>, epoch: u8) -> Result<u64> {
        require!(epoch >= 1 && epoch <= 4, ErrorCode::InvalidEpoch);
        let epoch_index = (epoch - 1) as usize;
        let gm_account = &ctx.accounts.gm_account;
        Ok(gm_account.staked_amount[epoch_index])
    }
}

#[derive(Accounts)]
pub struct GetStakedAmount<'info> {
    #[account(
        seeds = [b"gm_account", user.key().as_ref()],
        bump,
    )]
    pub gm_account: Account<'info, GreetingAccount>,
    pub user: Signer<'info>,
}

#[derive(Accounts)]
pub struct InitializeGmAccount<'info> {
    #[account(
        init,
        payer = user,
        space = 8 + 32 + 8 + (8 * 4) + (8 * 4),
        seeds = [b"gm_account", user.key().as_ref()],
        bump
    )]
    pub gm_account: Account<'info, GreetingAccount>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CreateUserATA<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
    pub mint: Account<'info, token::Mint>,
    #[account(
        init_if_needed,
        payer = user,
        associated_token::mint = mint,
        associated_token::authority = user
    )]
    pub user_token_account: Account<'info, TokenAccount>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct InitializeStakingWallet<'info> {
    #[account(
        init,
        payer = user,
        seeds = [b"staking_wallet"],
        bump,
        token::mint = mint,
        token::authority = staking_wallet,
    )]
    pub staking_wallet: Account<'info, TokenAccount>,
    pub mint: Account<'info, token::Mint>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct StakeTokens<'info> {
    #[account(mut)]
    pub gm_account: Account<'info, GreetingAccount>,
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(mut)]
    pub user_token_account: Account<'info, TokenAccount>,
    #[account(
        mut,
        seeds = [b"staking_wallet"],
        bump,
    )]
    pub staking_wallet: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
    pub clock: Sysvar<'info, Clock>,
}

#[derive(Accounts)]
pub struct UnstakeTokens<'info> {
    #[account(mut)]
    pub gm_account: Account<'info, GreetingAccount>,
    #[account(mut)]
    pub user: Signer<'info>,
    #[account(
        mut,
        constraint = user_token_account.owner == user.key() @ ErrorCode::InvalidUserTokenAccount,
        constraint = user_token_account.mint == staking_wallet.mint @ ErrorCode::InvalidMint
    )]
    pub user_token_account: Account<'info, TokenAccount>,
    #[account(
        mut,
        seeds = [b"staking_wallet"],
        bump,
    )]
    pub staking_wallet: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
    pub clock: Sysvar<'info, Clock>,
}

#[account]
pub struct GreetingAccount {
    pub name: String,
    pub first_greeting_time: i64,
    pub staked_amount: [u64; 4],
    pub last_stake_time: [i64; 4],
}

#[error_code]
pub enum ErrorCode {
    #[msg("The provided mint does not match the expected mint")]
    InvalidMint,
    #[msg("The user token account does not match the expected owner")]
    InvalidUserTokenAccount,
    #[msg("Insufficient staked amount")]
    InsufficientStakedAmount,
    #[msg("Numeric overflow")]
    NumericOverflow,
    #[msg("Invalid epoch")]
    InvalidEpoch,
    #[msg("Staking period has not ended")]
    StakingPeriodNotEnded,
}
