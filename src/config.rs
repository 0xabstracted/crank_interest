pub struct ClientConfig {
    pub keypair: Keypair,
    pub rpc_url: String,
}


pub fn setup_client(sugar_config: &SugarConfig) -> Result<SugarClient> {
    let rpc_url = sugar_config.rpc_url.clone();
    let ws_url = rpc_url.replace("http", "ws");
    let cluster = Cluster::Custom(rpc_url, ws_url);

    let key_bytes = sugar_config.keypair.to_bytes();
    let signer = Rc::new(Keypair::from_bytes(&key_bytes)?);

    let opts = CommitmentConfig::confirmed();
    Ok(Client::new_with_options(cluster, signer, opts))
}
