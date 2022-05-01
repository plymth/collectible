use scrypto::prelude::*;

#[derive(NonFungibleData)]
struct CollectibleMember {
    #[scrypto(mutable)]
    username: String,
    #[scrypto(mutable)]
    avatar: String,
}

#[derive(TypeId, Encode, Decode, Describe)]
pub enum CollectibleStatus {
    Available,
    Sold,
}

#[derive(NonFungibleData)]
struct CollectibleNft {
    name: String,
    description: String,
    image: String,
    price: Decimal,
    status: CollectibleStatus,
}

#[derive(NonFungibleData)]
struct CollectibleProof {
    collectible_nft_id: NonFungibleId,
    claimable_xrd: Decimal,
}

blueprint! {
    struct Collectible {
        /// A vault that holds the collectible minter badge
        collectible_minter: Vault,
        /// The resource address for membership badges
        collectible_member_resource_address: ResourceAddress,
        /// The resource address for a collectible nft
        collectible_nft_resource_address: ResourceAddress,
        /// The resource address for a collectible proof
        collectible_proof_resource_address: ResourceAddress,
        /// A vault that holds all collectible nfts minted
        collectible_nfts: Vault,
        /// A mapping of collectible proof -> collectible nft to verify ownership
        collectible_proofs: HashMap<NonFungibleId, NonFungibleId>,
        /// A vault that holds all xrd payments received 
        collected_xrd: Vault,
        /// The fee payable when a collectible nft is sold
        collectible_fee: Decimal
    }

    impl Collectible {
        pub fn instantiate_component() -> ComponentAddress {
            // Create collectible minter badge
            let collectible_minter: Bucket = ResourceBuilder::new_fungible()
                .divisibility(DIVISIBILITY_NONE)
                .initial_supply(1);
            
            // Create collectible member resource
            let collectible_member_resource_address: ResourceAddress = ResourceBuilder::new_non_fungible()
                .metadata("name", "Collectible Membership Badge")
                .mintable(rule!(require(collectible_minter.resource_address())), LOCKED)
                .no_initial_supply();
            
            // Create collectible nft resource
            let collectible_nft_resource_address: ResourceAddress = ResourceBuilder::new_non_fungible()
                .mintable(rule!(require(collectible_minter.resource_address())), LOCKED)
                .no_initial_supply();
            
            // Create collectible proof resource
            let collectible_proof_resource_address: ResourceAddress = ResourceBuilder::new_non_fungible()
                .mintable(rule!(require(collectible_minter.resource_address())), LOCKED)
                .burnable(rule!(require(collectible_minter.resource_address())), LOCKED)
                .updateable_non_fungible_data(rule!(require(collectible_minter.resource_address())), LOCKED)
                .no_initial_supply();
            
            // Instantiate component
            Self {
                collectible_minter: Vault::with_bucket(collectible_minter),
                collectible_member_resource_address,
                collectible_nft_resource_address,
                collectible_proof_resource_address,
                collectible_nfts: Vault::new(collectible_nft_resource_address),
                collectible_proofs: HashMap::new(),
                collected_xrd: Vault::new(RADIX_TOKEN),
                collectible_fee: dec!("0.025")
            }
            .instantiate()
            .globalize()
        }

        /// Returns a collectible member resource badge
        ///
        /// # Arguments
        /// 
        /// * `username` - An alphanumeric string which will be displayed on the user's account
        /// * `avatar` - A valid url string to an image that will be displayed on the user's account
        pub fn create_account(&mut self, username: String, avatar: String) -> Bucket {
            let badge  = self.collectible_minter.authorize(|| {
                let collectible_member_resource_manager: &ResourceManager = borrow_resource_manager!(self.collectible_member_resource_address);
                collectible_member_resource_manager.mint_non_fungible(&NonFungibleId::random(), CollectibleMember{ username, avatar })
            });

            badge
        }

        #[allow(unused_variables)]
        /// Returns a new minted collectible nft
        /// 
        /// # Arguments
        /// 
        /// * `collectible_member_resource_address` - The collectible member resource address
        /// * `name` - The name of the collectible nft
        /// * `description` - A description of the collectible nft
        /// * `image` - A url to an image that represents the collectible nft
        /// * `price` - The price of the collectible nft
        pub fn mint_collectible_nft(
            &mut self,
            collectible_member_resource_address: Proof,
            name: String,
            description: String,
            image: String,
            price: Decimal
        ) -> Bucket {
            // Mint a new Collectible NFT
            let nft = self.collectible_minter.authorize(|| {
                let collectible_nft_resource_manager: &ResourceManager = borrow_resource_manager!(self.collectible_nft_resource_address);
                collectible_nft_resource_manager.mint_non_fungible(&NonFungibleId::random(), CollectibleNft{ name, description, image, price, status: CollectibleStatus::Available })
            });

            // Get the Collectible NFT ID
            let nft_id = nft.non_fungible::<CollectibleNft>().id();

            // Get the Collectible NFT data
            let nft_data: CollectibleNft = nft.non_fungible().data();

            // Mint a new Collectible Proof
            let nft_proof = self.collectible_minter.authorize(|| {
                let collectible_proof_resource_manager: &ResourceManager = borrow_resource_manager!(self.collectible_proof_resource_address);

                // Calculate the claimable xrd
                let claimable_xrd: Decimal = self.collectible_fee * nft_data.price;

                collectible_proof_resource_manager.mint_non_fungible(&NonFungibleId::random(), CollectibleProof{ collectible_nft_id: nft_id, claimable_xrd })
            });

            // Get the Collectible Proof ID
            let nft_proof_id = nft_proof.non_fungible::<CollectibleProof>().id();

            // Get the Collectible NFT ID
            let nft_id = nft.non_fungible::<CollectibleNft>().id();

            // Create a mapping for Collectible Proof -> Collectible NFT to verify ownership
            self.collectible_proofs.insert(nft_proof_id, nft_id);

            // Store the Collectible NFT inside the collectible vault
            self.collectible_nfts.put(nft);

            // Return the Collectible Proof
            nft_proof
        }

        pub fn redeem_funds_for_collectible_nft(
            &mut self,
            collectible_proof: Bucket,
            key: NonFungibleId,
        ) -> Bucket {
            // Get the ID of the Collectible Proof
            let collectible_proof_id = collectible_proof.non_fungible::<CollectibleProof>().id();

            // Check if a valid Collectible Proof has been provided
            assert!(self.collectible_proofs.contains_key(&collectible_proof_id), "Invalid badge provided");

            // Get the Collectible NFT data
            let nft_data: CollectibleNft = self.collectible_minter.authorize(|| {
                let collectible_member_resource_manager: &ResourceManager = borrow_resource_manager!(self.collectible_nft_resource_address);
                collectible_member_resource_manager.get_non_fungible_data(&key)
            });

            match nft_data.status {
                CollectibleStatus::Sold => {
                    // Burn the Collectible Proof it the Collectible NFT has been sold
                    self.collectible_minter.authorize(|| {
                        collectible_proof.burn();
                    });
                    // Return xrd funds
                    self.collected_xrd.take(nft_data.price)
                },
                // Return the Collectible Proof is the Collectible NFT is still available
                CollectibleStatus::Available => collectible_proof
            }
        }

        #[allow(unused_variables)]
        pub fn buy_collectible_nft(
            &mut self,
            collectible_member_resource_address: Proof,
            key: NonFungibleId,
            mut payment: Bucket
        ) -> (Bucket, Bucket) {
            let nft_data: CollectibleNft = self.collectible_minter.authorize(|| {
                let collectible_member_resource_manager: &ResourceManager = borrow_resource_manager!(self.collectible_nft_resource_address);
                collectible_member_resource_manager.get_non_fungible_data(&key)
            });

            self.collected_xrd.put(payment.take(nft_data.price));

            let nft = self.collectible_nfts.take_non_fungible(&key);

            (nft, payment)
        }
    }
}
