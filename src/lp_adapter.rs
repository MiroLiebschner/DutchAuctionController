use scrypto::prelude::*;

//For now using the radiswap example to provide liquidity to
import! {
r#"
{
  "package": "01559905076cb3d4b9312640393a7bc6e1d4e491a8b1b62fa73a94",
  "name": "Radiswap",
  "functions": [
    {
      "name": "new",
      "inputs": [
        {
          "type": "Custom",
          "name": "scrypto::resource::Bucket",
          "generics": []
        },
        {
          "type": "Custom",
          "name": "scrypto::resource::Bucket",
          "generics": []
        },
        {
          "type": "Custom",
          "name": "scrypto::types::Decimal",
          "generics": []
        },
        {
          "type": "String"
        },
        {
          "type": "String"
        },
        {
          "type": "String"
        },
        {
          "type": "Custom",
          "name": "scrypto::types::Decimal",
          "generics": []
        }
      ],
      "output": {
        "type": "Tuple",
        "elements": [
          {
            "type": "Custom",
            "name": "scrypto::core::Component",
            "generics": []
          },
          {
            "type": "Custom",
            "name": "scrypto::resource::Bucket",
            "generics": []
          }
        ]
      }
    }
  ],
  "methods": [
    {
      "name": "add_liquidity",
      "mutability": "Immutable",
      "inputs": [
        {
          "type": "Custom",
          "name": "scrypto::resource::Bucket",
          "generics": []
        },
        {
          "type": "Custom",
          "name": "scrypto::resource::Bucket",
          "generics": []
        }
      ],
      "output": {
        "type": "Tuple",
        "elements": [
          {
            "type": "Custom",
            "name": "scrypto::resource::Bucket",
            "generics": []
          },
          {
            "type": "Custom",
            "name": "scrypto::resource::Bucket",
            "generics": []
          }
        ]
      }
    },
    {
      "name": "remove_liquidity",
      "mutability": "Immutable",
      "inputs": [
        {
          "type": "Custom",
          "name": "scrypto::resource::Bucket",
          "generics": []
        }
      ],
      "output": {
        "type": "Tuple",
        "elements": [
          {
            "type": "Custom",
            "name": "scrypto::resource::Bucket",
            "generics": []
          },
          {
            "type": "Custom",
            "name": "scrypto::resource::Bucket",
            "generics": []
          }
        ]
      }
    },
    {
      "name": "swap",
      "mutability": "Immutable",
      "inputs": [
        {
          "type": "Custom",
          "name": "scrypto::resource::Bucket",
          "generics": []
        }
      ],
      "output": {
        "type": "Custom",
        "name": "scrypto::resource::Bucket",
        "generics": []
      }
    },
    {
      "name": "get_pair",
      "mutability": "Immutable",
      "inputs": [],
      "output": {
        "type": "Tuple",
        "elements": [
          {
            "type": "Custom",
            "name": "scrypto::types::Address",
            "generics": []
          },
          {
            "type": "Custom",
            "name": "scrypto::types::Address",
            "generics": []
          }
        ]
      }
    }
  ]
}
"#
}

blueprint! {
    struct LPAdapter {
        radiswap: Radiswap,
    }

    impl LPAdapter {
        pub fn new(
            a_tokens: Bucket,
            b_tokens: Bucket,
        ) -> (Component, Address, Bucket) {
            let a_tokens_md = a_tokens.resource_def().metadata();
            let b_tokens_md = b_tokens.resource_def().metadata();
            let a_token_sym = a_tokens_md.get("symbol").unwrap();
            let b_token_sym = b_tokens_md.get("symbol").unwrap();

            let lp_name = format!("{}/{} Pool", &a_token_sym, &b_token_sym);
            let lp_symbol = format!("dex-{}-{}", a_token_sym, b_token_sym);
            let lp_url = "localhost".to_string();
            let lp_fee = Decimal::from_str("0.003").unwrap();
            let lp_amount = a_tokens.amount();

            let (radiswap, lp_token_buck) =  Radiswap::new(
                    a_tokens,
                    b_tokens,
                    lp_amount,
                    lp_symbol,
                    lp_name,
                    lp_url,
                    lp_fee
                );

            let radiswap_addr = radiswap.address();

            let lp_adapter = Self {
                radiswap: radiswap.into()
            }
            .instantiate();

            (lp_adapter, radiswap_addr, lp_token_buck)
        }
    }
}
