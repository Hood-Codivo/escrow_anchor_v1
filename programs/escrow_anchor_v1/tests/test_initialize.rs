use {
    anchor_lang::{
        AccountDeserialize, InstructionData, ToAccountMetas, solana_program::{
            instruction::Instruction, program_pack::Pack, pubkey::Pubkey, system_instruction,
            system_program,
        }
    },
    anchor_spl::{
        associated_token::{self, spl_associated_token_account::{self, instruction}},
        token::spl_token,
    },
    litesvm::LiteSVM,
    solana_keypair::Keypair,
    solana_message::Message,
    solana_signer::Signer,
    solana_transaction::Transaction,
};

const SEED: u64 = 123;
const DEPOSIT: u64 = 10_000_000;
const RECEIVE: u64 = 10_000_000;

fn setup() -> (LiteSVM, Keypair) {
    let program_id = escrow_anchor_v1::id();
    let payer = Keypair::new();
    let mut svm = LiteSVM::new();
    let bytes = include_bytes!("../../../target/deploy/escrow_anchor_v1.so");

    svm.add_program(program_id, bytes).unwrap();
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();

    (svm, payer)
}

fn send_ixs(svm: &mut LiteSVM, payer: &Keypair, ixs: &[Instruction], signers: &[&Keypair]) {
    let message = Message::new(ixs, Some(&payer.pubkey()));
    let transaction = Transaction::new(signers, message, svm.latest_blockhash());

   let result = svm.send_transaction(transaction).unwrap();
   println!("{:?}", result.signature);
    println!("compute unit consumed: {:?}", result.compute_units_consumed)
}

fn create_mint(svm: &mut LiteSVM, payer: &Keypair, authority: &Pubkey, decimals: u8) -> Pubkey {
    let mint = Keypair::new();
    let mint_len = spl_token::state::Mint::LEN;

    let create_mint_account_ix = system_instruction::create_account(
        &payer.pubkey(),
        &mint.pubkey(),
        svm.minimum_balance_for_rent_exemption(mint_len),
        mint_len as u64,
        &spl_token::ID,
    );
    let initialize_mint_ix = spl_token::instruction::initialize_mint2(
        &spl_token::ID,
        &mint.pubkey(),
        authority,
        None,
        decimals,
    )
    .unwrap();

    send_ixs(
        svm,
        payer,
        &[create_mint_account_ix, initialize_mint_ix],
        &[payer, &mint],
    );

    mint.pubkey()
}

fn create_associated_token_account(
    svm: &mut LiteSVM,
    payer: &Keypair,
    owner: &Pubkey,
    mint: &Pubkey,
) -> Pubkey {
    let ata = associated_token::get_associated_token_address(owner, mint);
    let create_ata_ix = spl_associated_token_account::instruction::create_associated_token_account(
        &payer.pubkey(),
        owner,
        mint,
        &spl_token::ID,
    );

    send_ixs(svm, payer, &[create_ata_ix], &[payer]);

    ata
}

fn mint_to(svm: &mut LiteSVM, payer: &Keypair, mint: &Pubkey, token_account: &Pubkey, amount: u64) {
    let mint_to_ix = spl_token::instruction::mint_to(
        &spl_token::ID,
        mint,
        token_account,
        &payer.pubkey(),
        &[],
        amount,
    )
    .unwrap();

    send_ixs(svm, payer, &[mint_to_ix], &[payer]);
}

#[test]
fn test_make_refund() {
    let (mut svm, payer) = setup();
    let maker = payer.pubkey();

    let mint_a = create_mint(&mut svm, &payer, &maker, 6);
    let mint_b = create_mint(&mut svm, &payer, &maker, 6);
    let maker_ata_a = create_associated_token_account(&mut svm, &payer, &maker, &mint_a);

    let (escrow, _) = Pubkey::find_program_address(
        &[b"escrow", maker.as_ref(), &SEED.to_le_bytes()],
        &escrow_anchor_v1::id(),
    );
    let vault = associated_token::get_associated_token_address(&escrow, &mint_a);

    mint_to(&mut svm, &payer, &mint_a, &maker_ata_a, 1_000_000_000);

    let make_ix = Instruction {
        program_id: escrow_anchor_v1::id(),
        accounts: escrow_anchor_v1::accounts::Make {
            maker,
            mint_a,
            mint_b,
            maker_ata_a,
            escrow,
            vault,
            token_program: spl_token::ID,
            associated_token_program: associated_token::ID,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
        data: escrow_anchor_v1::instruction::Make {
            seed: SEED,
            deposit: DEPOSIT,
            receive: RECEIVE,
        }
        .data(),
    };

    send_ixs(&mut svm, &payer, &[make_ix], &[&payer]);

    let vault_account = svm.get_account(&vault).unwrap();
    let vault_data = spl_token::state::Account::unpack(&vault_account.data).unwrap();
    assert_eq!(vault_data.amount, DEPOSIT);
    assert_eq!(vault_data.owner, escrow);
    assert_eq!(vault_data.mint, mint_a);

    let escrow_account = svm.get_account(&escrow).unwrap();
    let mut escrow_account_data = escrow_account.data.as_slice();
    let escrow_data =
        escrow_anchor_v1::state::Escrow::try_deserialize(&mut escrow_account_data).unwrap();
    assert_eq!(escrow_data.seed, SEED);
    assert_eq!(escrow_data.maker, maker);
    assert_eq!(escrow_data.mint_a, mint_a);
    assert_eq!(escrow_data.mint_b, mint_b);
    assert_eq!(escrow_data.send, DEPOSIT);
    assert_eq!(escrow_data.receive, RECEIVE);

    let refund_ix = Instruction {
        program_id: escrow_anchor_v1::id(),
        accounts: escrow_anchor_v1::accounts::Refund {
            maker,
            mint_a,
            maker_ata_a,
            escrow,
            value: vault,
            token_program: spl_token::ID,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
        data: escrow_anchor_v1::instruction::Refund {}.data(),
    };

    send_ixs(&mut svm, &payer, &[refund_ix], &[&payer]);

    assert!(svm.get_account(&escrow).is_none());
    assert!(svm.get_account(&vault).is_none());
}


#[test]
fn test_make_and_refund() {
    let (mut svm, payer) = setup();
    let maker = payer.pubkey();

    let mint_a = create_mint(&mut svm, &payer, &maker, 6);
    let mint_b = create_mint(&mut svm, &payer, &maker, 6);
    let maker_ata_a = create_associated_token_account(&mut svm, &payer, &maker, &mint_a);

    let escrow = Pubkey::find_program_address(
        &[b"escrow", maker.as_ref(), &SEED.to_le_bytes()],
        &escrow_anchor_v1::id(),
    )
    .0;
    let vault = associated_token::get_associated_token_address(&escrow, &mint_a);

    mint_to(&mut svm, &payer, &mint_a, &maker_ata_a, 1_000_000_000);

    let make_ix = Instruction {
        program_id: escrow_anchor_v1::id(),
        accounts: escrow_anchor_v1::accounts::Make {
            maker,
            mint_a,
            mint_b,
            maker_ata_a,
            escrow,
            vault,
            token_program: spl_token::ID,
            associated_token_program: associated_token::ID,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
        data: escrow_anchor_v1::instruction::Make {
            seed: SEED,
            deposit: DEPOSIT,
            receive: RECEIVE,
        }
        .data(),
    };

    send_ixs(&mut svm, &payer, &[make_ix], &[&payer]);

    let vault_account = svm.get_account(&vault).unwrap();
    let vault_data = spl_token::state::Account::unpack(&vault_account.data).unwrap();
    assert_eq!(vault_data.amount, DEPOSIT);
    assert_eq!(vault_data.owner, escrow);
    assert_eq!(vault_data.mint, mint_a);

    let escrow_account = svm.get_account(&escrow).unwrap();
    let mut escrow_account_data = escrow_account.data.as_slice();
    let escrow_data =
        escrow_anchor_v1::state::Escrow::try_deserialize(&mut escrow_account_data).unwrap();
    assert_eq!(escrow_data.seed, SEED);
    assert_eq!(escrow_data.maker, maker);
    assert_eq!(escrow_data.mint_a, mint_a);
    assert_eq!(escrow_data.mint_b, mint_b);
    assert_eq!(escrow_data.send, DEPOSIT);
    assert_eq!(escrow_data.receive, RECEIVE);

    let refund_ix = Instruction {
        program_id: escrow_anchor_v1::id(),
        accounts: escrow_anchor_v1::accounts::Refund {
            maker,
            mint_a,
            maker_ata_a,
            escrow,
            value: vault,
            token_program: spl_token::ID,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
        data: escrow_anchor_v1::instruction::Refund {}.data(),
    };

    send_ixs(&mut svm, &payer, &[refund_ix], &[&payer]);

    assert!(svm.get_account(&escrow).is_none());
    assert!(svm.get_account(&vault).is_none());
}
