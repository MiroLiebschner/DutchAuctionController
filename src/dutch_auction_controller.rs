use scrypto::prelude::*;

use crate::lp_adapter::LPAdapter;

#[derive(Debug, sbor::Decode, sbor::Encode, sbor::Describe, sbor::TypeId)]
struct Offering {
    clearing_badge: ResourceDef,
    payment_vault: Vault,
    sales_vault: Vault,
    token_starting_price: Decimal,
    decay_rate: Decimal,
    start_epoch: u64,
    total_epochs_to_run: u64,
    percentage_liquidity: Decimal,
    amount_of_tokens_offered: Decimal,
    rate_limit_last_epoch: u64,
    liquidity_provided: bool,
}

blueprint! {
    struct DutchAuctionController {
        offerings: HashMap<u32, Offering>,
        admin_badge_rd: ResourceDef,
        cur_id: u32,
        contract_active: bool,
        rate_limit: Decimal
    }

    impl DutchAuctionController {
        pub fn new() -> (Component, Bucket) {
            let admin_badge = ResourceBuilder::new()
                .metadata("name", "DutchAuctionController")
                .new_badge_fixed(1);

            let dutch_auction_controller = Self {
                offerings: HashMap::new(),
                admin_badge_rd: admin_badge.resource_def(),
                cur_id: 0,
                contract_active: true,
                rate_limit: Decimal::from_str("25000").unwrap()
            }
            .instantiate();

            (dutch_auction_controller, admin_badge)
        }

        //Circuit Breaker, for security-purposes
        #[auth(admin_badge_rd)]
        pub fn toggle_circuit(&mut self) {
            self.contract_active = !self.contract_active;
        }

        #[auth(admin_badge_rd)]
        pub fn create_offering(
            &mut self,
            tokens: Bucket,
            token_starting_price_string: String,
            decay_rate_string: String,
            start_epoch: u64,
            total_epochs_to_run: u64,
            percentage_liquidity_string: String
        ) -> Bucket {
            let clearing_badge_buck = ResourceBuilder::new()
                .metadata("name", "RadStarter Clearing Badge")
                .metadata("id", self.cur_id.to_string())
                .new_badge_fixed(1);
            let clearing_badge = clearing_badge_buck.resource_def();
            let token_starting_price = Decimal::from_str(&token_starting_price_string).unwrap();
            let decay_rate = Decimal::from_str(&decay_rate_string).unwrap();

            let amount_of_tokens_offered = tokens.amount();
            let percentage_liquidity = Decimal::from_str(&percentage_liquidity_string).unwrap() / 100;

            //Check that token price can't become negative
            let total_epochs_to_run_dec: Decimal = total_epochs_to_run.into();
            let max_epochs = token_starting_price / decay_rate;

            scrypto_assert!(
                total_epochs_to_run_dec < max_epochs,
                "To many epochs for current decay rate, price will go negative"
            );

            //Check if percentage_liquidity is sane
            let percentage_min = Decimal::from_str("0").unwrap();
            let percentage_max = Decimal::from_str("0.51").unwrap();

            scrypto_assert!(
               percentage_liquidity >= percentage_min && percentage_liquidity <= percentage_max,
                "Choose a liquidity percentage between 1 and 51 percent"
            );

            let offering = Offering {
                clearing_badge,
                payment_vault: Vault::new(RADIX_TOKEN),
                sales_vault: Vault::with_bucket(tokens),
                token_starting_price,
                decay_rate,
                start_epoch,
                total_epochs_to_run,
                percentage_liquidity,
                amount_of_tokens_offered,
                rate_limit_last_epoch: 0,
                liquidity_provided: false
            };

            self.offerings.insert(self.cur_id, offering);
            self.cur_id += 1;

            clearing_badge_buck
        }


        pub fn buy(&mut self, id: u32, payment_buck: Bucket) -> (Bucket, Bucket) {
            scrypto_assert!(
                self.contract_active,
                "Contract is currently not active"
            );

            let offering = self.offerings.get(&id).unwrap();
            let epochs_passed = Context::current_epoch() - offering.start_epoch;

            scrypto_assert!(
                Context::current_epoch() >= offering.start_epoch,
                "Auction hasn't started"
            );
            scrypto_assert!(
                offering.total_epochs_to_run >= epochs_passed,
                "Auction has ended"
            );

            scrypto_assert!(
                offering.payment_vault.resource_def() == payment_buck.resource_def(),
                "You have paid with the incorrect token, use XRD"
            );

            let min_tokens = offering.percentage_liquidity * offering.amount_of_tokens_offered;
            scrypto_assert!(
                !offering.sales_vault.is_empty() && offering.sales_vault.amount() > min_tokens,
                "All tokens have been sold out"
            );

            //Calculate token price
            let epochs_passed_dec: Decimal = epochs_passed.into();
            let token_price = offering.token_starting_price - epochs_passed_dec * offering.decay_rate;
             // The amount of purchased tokens
            let mut amount_ret = payment_buck.amount() / token_price;

            // Calculate change
            let change_buck: Bucket = Bucket::new(payment_buck.resource_def());
            if amount_ret > offering.sales_vault.amount()  {
                let unfilled_amount = amount_ret - offering.sales_vault.amount();
                change_buck.put(payment_buck.take(unfilled_amount * token_price));
                amount_ret = payment_buck.amount() / token_price;
            }


            offering.payment_vault.put(payment_buck);
            (offering.sales_vault.take(amount_ret), change_buck)
        }

        pub fn clear_offering(&mut self, badge: BucketRef) -> (Bucket, Bucket) {
            scrypto_assert!(
                self.contract_active,
                "Contract is currently not active"
            );

            let id = badge
                .resource_def()
                .metadata()
                .get("id")
                .unwrap()
                .parse::<u32>()
                .unwrap();

            let offering = self.offerings.get_mut(&id).unwrap();

            scrypto_assert!(
                badge.resource_def() == offering.clearing_badge,
                "Wrong badge"
            );

            let epochs_passed = Context::current_epoch() - offering.start_epoch;


            scrypto_assert!(
                epochs_passed > offering.total_epochs_to_run,
                "Auction hasn't finished"
            );

           scrypto_assert!(
               offering.rate_limit_last_epoch < Context::current_epoch() + 24,
               "Already withdrawn in the last 24 epochs, rate limited"
           );

           offering.rate_limit_last_epoch = Context::current_epoch();

           let zero_percent = Decimal::from_str("0").unwrap();
           scrypto_assert!(
               offering.liquidity_provided || offering.percentage_liquidity == zero_percent,
               "You need to call provide_liquidity before you can withdraw the tokens"
           );

            let ret_stable_buck: Bucket;
            let ret_token_buck: Bucket;

            if offering.sales_vault.amount() < self.rate_limit {
                ret_token_buck = offering.sales_vault.take_all();
            } else {
                ret_token_buck = offering.sales_vault.take(self.rate_limit);
            }

            if offering.payment_vault.amount() < self.rate_limit {
                ret_stable_buck = offering.payment_vault.take_all();
            } else {
                ret_stable_buck = offering.payment_vault.take(self.rate_limit);
            }


            (ret_stable_buck, ret_token_buck)
        }

        pub fn provide_liquidity(&mut self, badge: BucketRef) -> (Bucket, Address) {
            scrypto_assert!(
                self.contract_active,
                "Contract is currently not active"
            );

            let id = badge
                .resource_def()
                .metadata()
                .get("id")
                .unwrap()
                .parse::<u32>()
                .unwrap();

            let offering = self.offerings.get_mut(&id).unwrap();

            scrypto_assert!(
                badge.resource_def() == offering.clearing_badge,
                "Wrong badge"
            );

            let epochs_passed = Context::current_epoch() - offering.start_epoch;


            scrypto_assert!(
                epochs_passed > offering.total_epochs_to_run,
                "Auction hasn't finished"
            );

            let token_buck = offering.sales_vault.take(
                    offering.amount_of_tokens_offered * offering.percentage_liquidity
            );
            let stable_buck = offering.payment_vault.take(
                offering.payment_vault.amount() * offering.percentage_liquidity
            );

            let (_lp_adapter, lp_address, lp_tokens) = LPAdapter::new(token_buck, stable_buck);

            offering.liquidity_provided = true;

            (lp_tokens, lp_address)
        }

        //Function for when something goes wrong with LP-provisioning so the tokens don't stay
        //locked forever

        #[auth(admin_badge_rd)]
        pub fn set_lp_provided(&mut self, id: u32) {
            let offering = self.offerings.get_mut(&id).unwrap();
            offering.liquidity_provided = true;

        }
    }
}
