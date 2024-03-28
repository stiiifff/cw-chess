#[cfg(test)]
mod tests {
    use crate::helpers::CwChessContract;
    use crate::msg::InstantiateMsg;
    use cosmwasm_std::{Addr, Coin, Empty};
    // use cosmwasm_std::{Addr, Coin, Empty, Uint128};
    use cw_multi_test::{App, AppBuilder, Contract, ContractWrapper, Executor};

    pub fn chess_contract() -> Box<dyn Contract<Empty>> {
        let contract = ContractWrapper::new(
            crate::contract::execute,
            crate::contract::instantiate,
            crate::contract::query,
        );
        Box::new(contract)
    }

    // const USER: &str = "USER";
    const ADMIN: &str = "ADMIN";
    const NATIVE_DENOM: &str = "untrn";

    fn mock_app() -> App {
        AppBuilder::new().build(|_router, _, _storage| {
            // router
            //     .bank
            //     .init_balance(
            //         storage,
            //         &Addr::unchecked(USER),
            //         vec![Coin {
            //             denom: NATIVE_DENOM.to_string(),
            //             amount: Uint128::new(1),
            //         }],
            //     )
            //     .unwrap();
        })
    }

    fn proper_instantiate() -> (App, CwChessContract) {
        let mut app = mock_app();
        let cw_chess_id = app.store_code(chess_contract());

        let msg = InstantiateMsg {
            min_bet: Coin::new(10, NATIVE_DENOM),
        };
        let cw_chess_contract_addr = app
            .instantiate_contract(
                cw_chess_id,
                Addr::unchecked(ADMIN),
                &msg,
                &[],
                "cw-chess",
                Some(Addr::unchecked(ADMIN).into_string()),
            )
            .unwrap();

        let cw_template_contract = CwChessContract(cw_chess_contract_addr);

        (app, cw_template_contract)
    }

    mod init {
        use super::*;

        #[test]
        fn instantiate() {
            let (_app, _contract) = proper_instantiate();
        }
    }
}
