use lite_program_test::ProgramTest;
use solana_program::{
    instruction::{AccountMeta, Instruction},
    message::Message,
    pubkey::Pubkey,
    rent::Rent,
};
use solana_sdk::{account::Account, signature::Keypair, signer::Signer, transaction::Transaction};

const COUNTER_PROGRAM_BYTES: &[u8] =
    include_bytes!("../../../../lite-svm/tests/programs_bytes/counter.so");
const NUM_GREETINGS: u8 = 255;

#[test]
pub fn integration_test() {
    let svm = ProgramTest::new();
    let payer_kp = Keypair::new();
    let payer_pk = payer_kp.pubkey();
    let program_id = Pubkey::new_unique();
    svm.store_program(program_id, COUNTER_PROGRAM_BYTES);

    svm.request_airdrop(&payer_pk, 1000000000);
    let blockhash = svm.get_latest_blockhash();
    let counter_address = Pubkey::new_unique();
    svm.get_bank_mut().accounts.add_account(
        counter_address,
        Account {
            lamports: 5,
            data: vec![0_u8; std::mem::size_of::<u32>()],
            owner: program_id,
            ..Default::default()
        }
        .into(),
    );
    assert_eq!(
        svm.get_account(&counter_address).data,
        0u32.to_le_bytes().to_vec()
    );
    let msg = Message::new_with_blockhash(
        &[Instruction {
            program_id,
            accounts: vec![AccountMeta::new(counter_address, false)],
            data: vec![0],
        }],
        Some(&payer_pk),
        &blockhash,
    );
    let tx = Transaction::new(&[&payer_kp], msg, blockhash);
    let num_greets = 100u32;
    for _ in 0..num_greets {
        let tx_res = svm.send_transaction(tx.clone()).unwrap();
        tx_res.result.unwrap();
        println!("tx_res.metadata.logs: {:?}", tx_res.metadata.logs);
    }
    assert_eq!(
        svm.get_account(&counter_address).data,
        num_greets.to_le_bytes().to_vec()
    );
}

fn make_tx(
    program_id: Pubkey,
    counter_address: Pubkey,
    payer_pk: &Pubkey,
    blockhash: solana_program::hash::Hash,
    payer_kp: &Keypair,
    deduper: u8,
) -> Transaction {
    let msg = Message::new_with_blockhash(
        &[Instruction {
            program_id,
            accounts: vec![AccountMeta::new(counter_address, false)],
            data: vec![0, deduper],
        }],
        Some(payer_pk),
        &blockhash,
    );
    Transaction::new(&[payer_kp], msg, blockhash)
}

fn add_program(bytes: &[u8], program_id: Pubkey, pt: &mut solana_program_test::ProgramTest) {
    pt.add_account(
        program_id,
        Account {
            lamports: Rent::default().minimum_balance(bytes.len()).max(1),
            data: bytes.to_vec(),
            owner: solana_sdk::bpf_loader::id(),
            executable: true,
            rent_epoch: 0,
        },
    );
}

fn counter_acc(program_id: Pubkey) -> solana_sdk::account::Account {
    Account {
        lamports: 5,
        data: vec![0_u8; std::mem::size_of::<u32>()],
        owner: program_id,
        ..Default::default()
    }
}

async fn do_program_test(program_id: Pubkey, counter_address: Pubkey) {
    let mut pt = solana_program_test::ProgramTest::default();
    add_program(COUNTER_PROGRAM_BYTES, program_id, &mut pt);
    let mut ctx = pt.start_with_context().await;
    ctx.set_account(&counter_address, &counter_acc(program_id));
    assert_eq!(
        ctx.banks_client
            .get_account(counter_address)
            .await
            .unwrap()
            .unwrap()
            .data,
        0u32.to_le_bytes().to_vec()
    );
    assert!(ctx
        .banks_client
        .get_account(program_id)
        .await
        .unwrap()
        .is_some());

    for deduper in 0..NUM_GREETINGS {
        let tx = make_tx(
            program_id,
            counter_address,
            &ctx.payer.pubkey(),
            ctx.last_blockhash,
            &ctx.payer,
            deduper,
        );
        let tx_res = ctx
            .banks_client
            .process_transaction_with_metadata(tx)
            .await
            .unwrap();
        tx_res.result.unwrap();
        println!("logs: {:?}", tx_res.metadata.unwrap().log_messages);
    }
    let fetched = ctx
        .banks_client
        .get_account(counter_address)
        .await
        .unwrap()
        .unwrap()
        .data[0];
    assert_eq!(fetched, NUM_GREETINGS);
}

#[test]
fn banks_client_test() {
    let program_id = Pubkey::new_unique();

    let counter_address = Pubkey::new_unique();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async { do_program_test(program_id, counter_address).await });
}
