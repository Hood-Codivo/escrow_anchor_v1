# Escrow Anchor v1

This project is a Solana escrow program built with Anchor. It lets a maker lock one SPL token in a vault, record what token amount they want in return, and later either complete the swap with a taker or refund the maker.

The program id configured for localnet is:

```text
FTYviMqzWVj9b9jbFSwiSrGmxLwo4MKEFSruLpdRYTHW
```

## What This Program Does

The escrow is built around three instructions:

- `make`: the maker creates an escrow, deposits token A into a vault, and records how much token B they want back.
- `take`: the taker accepts the escrow by sending token B to the maker and receiving token A from the vault.
- `refund`: the maker cancels the escrow and receives token A back from the vault.

The core idea is:

```text
maker deposits token A -> escrow vault
taker pays token B -> maker
vault releases token A -> taker
```

If no taker completes the trade, the maker can refund and close the escrow.

### `Make`

- `maker`: signer creating the escrow.
- `mint_a`: token mint being deposited by the maker.
- `mint_b`: token mint requested from the taker.
- `maker_ata_a`: maker's token A account.
- `escrow`: PDA storing escrow terms.
- `vault`: associated token account owned by the escrow PDA.
- `token_program`: SPL token program.
- `associated_token_program`: associated token account program.
- `system_program`: system program.

### `Refund`

- `maker`: signer reclaiming the deposited token A.
- `mint_a`: token mint stored in the vault.
- `maker_ata_a`: maker's token A account.
- `escrow`: escrow PDA account to close.
- `value`: vault token account holding token A.
- `token_program`: SPL token program.
- `system_program`: system program.

### `Take`

- `taker`: signer accepting the escrow.
- `maker`: original escrow creator.
- `mint_a`: token mint released from the vault.
- `mint_b`: token mint paid by the taker.
- `taker_ata_a`: taker's token A account.
- `taker_ata_b`: taker's token B account.
- `maker_ata_b`: maker's token B account.
- `escrow`: escrow PDA account.
- `vault`: token A vault.
- `token_program`: SPL token program.
- `associated_token_program`: associated token account program.
- `system_program`: system program.

## Tests

The Rust tests live in:

```text
programs/escrow_anchor_v1/tests/test_initialize.rs
```

They use LiteSVM to run the program locally without a validator.

Test helpers:

- `setup`: starts LiteSVM, loads the compiled program, and airdrops SOL to the payer.
- `send_ixs`: creates and sends transactions, then prints the signature and compute units.
- `create_mint`: creates and initializes an SPL token mint.
- `create_associated_token_account`: creates an ATA for a wallet and mint.
- `mint_to`: mints SPL tokens into a token account.

Current test coverage:

- Creates token A and token B mints.
- Creates the maker's token A ATA.
- Mints token A to the maker.
- Derives the escrow PDA and vault ATA.
- Sends the `make` instruction.
- Verifies the vault holds the deposited token A amount.
- Verifies escrow account state fields.
- Sends the `refund` instruction.
- Verifies the escrow and vault accounts are closed.

There are two make/refund tests:

- `test_make_refund`
- `test_make_and_refund`

Both tests verify the same main behavior: the maker can deposit token A into escrow, the escrow state is stored correctly, and the maker can refund and close the escrow.

The LiteSVM tests load the compiled program binary from:

```text
target/deploy/escrow_anchor_v1.so
```

Build the Anchor program first:

```bash
anchor build
```

Then run the tests:

```bash
cargo test
```

Or run just the integration test file:

```bash
cargo test --test test_initialize
```

Expected result after the program binary exists:

```text
2 passed; 0 failed
```
