use irohffi::peer_id_to_ed25519_public_key;


#[tokio::main]
async fn main() {
    let peer_id = "12D3KooWAGp6spceXhke53cDVafgcdgNku4AiSpB7gjag9AADqKw";  
    println!("Peer ID: {}", peer_id);
    if let Some(pub_key) = peer_id_to_ed25519_public_key(peer_id) {
        println!("Ed25519 Public Key: {:?}", pub_key);
    } else {
        println!("Failed to decode Peer ID or not an Ed25519 key.");
    }
}