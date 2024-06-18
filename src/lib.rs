pub mod address;
mod bip327;
mod bip340;
mod bip341;
mod bip350;
mod scalars;
mod scripts;

#[derive(Debug, PartialEq)]
pub
enum Network {
	Mainnet,
	Testnet,
	Regtest
}
