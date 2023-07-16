use {
    std::{
        rc::Rc,
        str::FromStr, 
        time::Duration,
        thread::sleep,
    },
    chrono::prelude::*,
    solana_program::{
        sysvar,
    },
    solana_sdk::{
        signer::Signer,
        pubkey::Pubkey,
    },
    solana_client::{
        rpc_response::Response,
        rpc_client::RpcClient,
    },
    anchor_client::{
        solana_sdk::{
            hash::Hash,
            compute_budget::ComputeBudgetInstruction,
            commitment_config::CommitmentConfig,
            signature::{keypair::Keypair, read_keypair_file},
        },
        Client, Cluster,
    },
    anyhow::{anyhow, Result, Error},
    savings_vault::accounts,
    spl_token::ID as TOKEN_PROGRAM_ID,
};


pub const SAVINGS_VAULT_PROGRAM_ID: &str = "HfJVM6Ayjajt9H58AZoCFqkCQQFehSeQfGQbi3crxT8W";

pub const KEYPAIR_PATH: &str = "/Users/0xabstracted/.config/solana/id.json";
pub const RPC_URL: &str = "https://api.devnet.solana.com";
pub const COMPUTE_UNITS: u32 = 400_000;


pub const SEED_SAVINGS_VAULT: &[u8] = b"savings_vault";
pub const SEED_SAVINGS_VAULT_TREASURY: &[u8] = b"savings_vault-treasury";
pub const SEED_INTEREST_DEPOSITOR_MANAGER: &[u8] = b"interest_depositor_manager";
pub const SEED_INTEREST_DEPOSITOR_TREASURY: &[u8] = b"interest_depositor_treasury";

pub struct ClientConfig {
    pub keypair: Keypair,
    pub rpc_url: String,
}

pub type SavingsVaultClient = Client<Rc<Keypair>>;

pub fn setup_client(config: &ClientConfig) -> Result<SavingsVaultClient> {
    let rpc_url = config.rpc_url.clone();
    let ws_url = rpc_url.replace("http", "ws");
    let cluster = Cluster::Custom(rpc_url, ws_url);

    let key_bytes = config.keypair.to_bytes();
    let signer = Rc::new(Keypair::from_bytes(&key_bytes)?);

    let opts = CommitmentConfig::confirmed();
    Ok(Client::new_with_options(cluster, signer, opts))
}


pub fn find_savings_vault_pda(mint: &Pubkey, wallet: &Pubkey) -> (Pubkey, u8) {
    let savings_vault_seeds = &[SEED_SAVINGS_VAULT, mint.as_ref(), wallet.as_ref()];
    let savings_vault_program_key: Pubkey  = Pubkey::from_str(SAVINGS_VAULT_PROGRAM_ID).unwrap();

    Pubkey::find_program_address(savings_vault_seeds, &savings_vault_program_key)
}


pub fn find_savings_vault_treasury_pda(savings_vault: &Pubkey) -> (Pubkey, u8) {
    let savings_vault_treasury_seeds = &[SEED_SAVINGS_VAULT_TREASURY, savings_vault.as_ref()];
    let savings_vault_program_key: Pubkey  = Pubkey::from_str(SAVINGS_VAULT_PROGRAM_ID).unwrap();

    Pubkey::find_program_address(savings_vault_treasury_seeds, &savings_vault_program_key)
}

pub fn find_interest_depositor_manager_pda(mint: &Pubkey) -> (Pubkey, u8) {
    let interest_depositor_manager_seeds = &[SEED_INTEREST_DEPOSITOR_MANAGER, mint.as_ref()];
    let savings_vault_program_key: Pubkey  = Pubkey::from_str(SAVINGS_VAULT_PROGRAM_ID).unwrap();

    Pubkey::find_program_address(interest_depositor_manager_seeds, &savings_vault_program_key)
}

pub fn find_interest_depositor_treasury_pda(interest_depositor_manager: &Pubkey) -> (Pubkey, u8) {
    let interest_depositor_treasury_seeds = &[SEED_INTEREST_DEPOSITOR_TREASURY, interest_depositor_manager.as_ref()];
    let savings_vault_program_key: Pubkey  = Pubkey::from_str(SAVINGS_VAULT_PROGRAM_ID).unwrap();

    Pubkey::find_program_address(interest_depositor_treasury_seeds, &savings_vault_program_key)
}

async fn crank_accrue_interest(
    client: &SavingsVaultClient,
    cranker: &Keypair,
    wallet: &Pubkey,
    mint: &Pubkey,
) -> Result<(), Error> {
        let savings_vault_program_key: Pubkey  = Pubkey::from_str(SAVINGS_VAULT_PROGRAM_ID).unwrap();

        let wallet = *wallet;
        let mint = *mint;
        let savings_vault: Pubkey = find_savings_vault_pda(&mint, &wallet).0;
        let savings_vault_treasury: Pubkey = find_savings_vault_treasury_pda(&savings_vault).0;  
        let interest_depositor_manager: Pubkey = find_interest_depositor_manager_pda(&mint).0;
        let interest_depositor_treasury: Pubkey = find_interest_depositor_treasury_pda(&interest_depositor_manager).0;
        let cranker_clone = cranker;
        let program = client.program(savings_vault_program_key);
        
        let accrue_ix = program
            .request()
            .accounts(
                accounts::AccrueInterest {
                    mint,
                    cranker: cranker_clone.pubkey(),
                    wallet,
                    savings_vault,
                    savings_vault_treasury,
                    interest_depositor_manager,
                    interest_depositor_treasury,
                    token_program: TOKEN_PROGRAM_ID,
                    clock: sysvar::clock::ID,
                });
                            
        let accrue_ix = accrue_ix.instructions()?;

        let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(COMPUTE_UNITS);

        let builder = program
            .request()
            .instruction(compute_ix)
            .instruction(accrue_ix[0].clone())
            .signer(cranker_clone);

        let _sig = builder.send();

        if let Err(_) | Ok(Response { value: None, .. }) = program
            .rpc()
            .get_account_with_commitment(&savings_vault, CommitmentConfig::processed())
        {
            let cluster_param = match get_cluster(program.rpc()).unwrap_or(Cluster::Mainnet) {
                Cluster::Devnet => "?devnet",
                _ => "",
            };
            return Err(anyhow!(
                "Savings vault account {} does not exist on cluster {}",
                savings_vault,
                cluster_param
            ));
        }


    Ok(())
}

/// Hash for devnet cluster
pub const DEVNET_HASH: &str = "EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG";

/// Hash for mainnet-beta cluster
pub const MAINNET_HASH: &str = "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d";

pub fn get_cluster(rpc_client: RpcClient) -> Result<Cluster> {
    let devnet_hash = Hash::from_str(DEVNET_HASH).unwrap();
    let mainnet_hash = Hash::from_str(MAINNET_HASH).unwrap();
    let genesis_hash = rpc_client.get_genesis_hash()?;

    Ok(if genesis_hash == devnet_hash {
        Cluster::Devnet
    } else if genesis_hash == mainnet_hash {
        Cluster::Mainnet
    } else {
        Cluster::Devnet
    })
}

#[tokio::main]
async fn main() {
    // let client = RpcClient::new(RPC_URL);
    let cranker = read_keypair_file(KEYPAIR_PATH).unwrap();
    let client = setup_client(&ClientConfig {
        keypair: cranker,
        rpc_url: RPC_URL.to_string(),
    }).unwrap();
    loop {
        let last_execution_time = Utc::now();

        let current_time = Utc::now();
        let duration_since_last_execution = current_time.signed_duration_since(last_execution_time);
        // get the wallets using the savings_vault protocol and mints supported from the database
        let wallet: Pubkey = Pubkey::from_str("TUAXRFzyLeXmG9wPLaMXt66jUagfrWmL9oGq4rMwjAu").unwrap();
        let mint: Pubkey = Pubkey::from_str("FmAFDKSPL61s8kQZCHwsZULA313pdHJ73PuBK4wePpNh").unwrap(); 
        
        if duration_since_last_execution.num_days() >= 30 {
            let cranker = read_keypair_file(KEYPAIR_PATH).unwrap();
            let _res = crank_accrue_interest(&client, &cranker, &wallet, &mint).await;
        }
        sleep(Duration::from_secs(60 * 60)); // check every hour
    }
}
