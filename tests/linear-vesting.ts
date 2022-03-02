import * as anchor from '@project-serum/anchor';
import { Program } from '@project-serum/anchor';
import { LinearVesting } from '../target/types/linear_vesting';

import {
  PublicKey,
  SystemProgram,
  Transaction,
  Connection,
  Commitment,
} from '@solana/web3.js';
import { TOKEN_PROGRAM_ID, Token } from '@solana/spl-token';
import { assert } from 'chai';
import NodeWallet from '@project-serum/anchor/dist/cjs/nodewallet';

const SPL_ASSOCIATED_TOKEN_ACCOUNT_PROGRAM_ID: PublicKey = new PublicKey(
  'ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL'
);

async function findAssociatedTokenAddress(
  walletAddress: PublicKey,
  tokenMintAddress: PublicKey
): Promise<PublicKey> {
  return (
    await PublicKey.findProgramAddress(
      [
        walletAddress.toBuffer(),
        TOKEN_PROGRAM_ID.toBuffer(),
        tokenMintAddress.toBuffer(),
      ],
      SPL_ASSOCIATED_TOKEN_ACCOUNT_PROGRAM_ID
    )
  )[0];
}

const sleep = (milliseconds: number) =>
  new Promise((resolve) => setTimeout(resolve, milliseconds));

describe('linear-vesting', () => {
  const provider = anchor.Provider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.LinearVesting as Program<LinearVesting>;

  let mint = null as Token;
  let ownerTokenAccount = null;

  const amount = 1000000 * 10 ** 9;

  const owner = (provider.wallet as NodeWallet).payer;
  const beneficiary = anchor.web3.Keypair.generate();
  const mintAuthority = owner;

  let vaultAuthority = null;
  let beneficiaryTokenAccount = null;
  let vestingAccount = null;
  let vaultAccount = null;

  it('Initialize vesting account', async () => {
    mint = await Token.createMint(
      provider.connection,
      mintAuthority,
      mintAuthority.publicKey,
      null,
      9,
      TOKEN_PROGRAM_ID
    );

    ownerTokenAccount = await mint.getOrCreateAssociatedAccountInfo(
      owner.publicKey
    );

    await mint.mintTo(
      ownerTokenAccount.address,
      mintAuthority.publicKey,
      [mintAuthority],
      amount
    );

    beneficiaryTokenAccount = await mint.getOrCreateAssociatedAccountInfo(
      beneficiary.publicKey
    );

    [vaultAccount] = await PublicKey.findProgramAddress(
      [
        Buffer.from(anchor.utils.bytes.utf8.encode('token-vault')),
        beneficiaryTokenAccount.address.toBuffer(),
      ],

      program.programId
    );

    [vaultAuthority] = await PublicKey.findProgramAddress(
      [Buffer.from(anchor.utils.bytes.utf8.encode('vault-authority'))],
      program.programId
    );

    [vestingAccount] = await PublicKey.findProgramAddress(
      [beneficiaryTokenAccount.address.toBuffer()],
      program.programId
    );

    const startTs = new anchor.BN(Date.now() / 1000);
    const cliffTs = new anchor.BN(0);
    const duration = new anchor.BN(100);

    await program.rpc.initialize(
      new anchor.BN(amount),
      startTs,
      cliffTs,
      duration,
      true,
      {
        accounts: {
          owner: owner.publicKey,
          beneficiary: beneficiary.publicKey,
          mint: mint.publicKey,
          beneficiaryAta: beneficiaryTokenAccount.address,
          vaultAccount: vaultAccount,
          ownerTokenAccount: ownerTokenAccount.address,
          vestingAccount: vestingAccount,
          systemProgram: anchor.web3.SystemProgram.programId,
          rent: anchor.web3.SYSVAR_RENT_PUBKEY,
          tokenProgram: TOKEN_PROGRAM_ID,
        },
        signers: [owner],
      }
    );

    let _vault = await mint.getAccountInfo(vaultAccount);
    let _owner = await mint.getAccountInfo(ownerTokenAccount.address);

    // check token has been transferred to vault
    assert.ok(_vault.amount.toNumber() == amount);
    assert.ok(_owner.amount.toNumber() === 0);

    // check that the vault's authority is the program
    assert.ok(vaultAuthority.equals(_vault.owner));

    let _vestingAccount = await program.account.vestingAccount.fetch(
      vestingAccount
    );

    // check the vesting account has the correct data

    assert.ok(_vestingAccount.totalDepositedAmount.toNumber() === amount);
    assert.ok(_vestingAccount.releasedAmount.toNumber() === 0);
    assert.ok(_vestingAccount.startTs.eq(startTs));
    assert.ok(_vestingAccount.cliffTs.eq(cliffTs));
    assert.ok(_vestingAccount.duration.eq(duration));
    assert.ok(_vestingAccount.revocable);
    assert.ok(_vestingAccount.beneficiary.equals(beneficiary.publicKey));
    assert.ok(_vestingAccount.owner.equals(owner.publicKey));
    assert.ok(_vestingAccount.mint.equals(mint.publicKey));
  });

  it('Withdraw', async () => {
    await sleep(3000);

    await program.rpc.withdraw({
      accounts: {
        beneficiary: beneficiary.publicKey,
        beneficiaryAta: beneficiaryTokenAccount.address,
        vaultAccount,
        vestingAccount,
        vaultAuthority,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      },
      signers: [beneficiary],
    });

    const _vault = await mint.getAccountInfo(vaultAccount);
    const _beneficiary = await mint.getAccountInfo(
      beneficiaryTokenAccount.address
    );
    const remaining_amount = ((1000000 * 98) / 100) * 10 ** 9;
    const beneficiary_amount = amount - remaining_amount;
    const _vestingAccount = await program.account.vestingAccount.fetch(
      vestingAccount
    );
    assert.ok(_vault.amount.toNumber() == remaining_amount);
    assert.ok(_beneficiary.amount.toNumber() == beneficiary_amount);
    assert.ok(_vestingAccount.releasedAmount.toNumber() === beneficiary_amount);
  });

  it('Revoke', async () => {
    await program.rpc.revoke({
      accounts: {
        owner: owner.publicKey,
        ownerTokenAccount: ownerTokenAccount.address,
        vaultAccount,
        vestingAccount,
        vaultAuthority,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      },
      signers: [owner],
    });

    const _vault = await mint.getAccountInfo(vaultAccount);
    const _vestingAccount = await program.account.vestingAccount.fetch(
      vestingAccount
    );
    assert.ok(_vault.amount.toNumber() == 0);
    assert.ok(_vestingAccount.revoked);
  });
});
