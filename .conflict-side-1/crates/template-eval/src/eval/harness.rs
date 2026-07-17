use libtest_mimic::Trial;

use crate::eval::{
    Arguments, EvalCase, Failed, live_config_from_env, run_contract, run_live, run_smoke,
};

pub fn run_case_suite(args: Arguments, cases: Result<Vec<EvalCase>, Failed>) -> ! {
    let trials = match cases {
        Ok(cases) => case_trials(cases),
        Err(err) => vec![Trial::test("setup::render_cases", move || Err(err))],
    };

    libtest_mimic::run(&args, trials).exit();
}

fn case_trials(cases: Vec<EvalCase>) -> Vec<Trial> {
    let mut trials = Vec::with_capacity(cases.len() * 3);
    for case in cases {
        let contract_case = case.clone();
        trials.push(Trial::test(
            format!("contract::{}", contract_case.name),
            move || run_contract(&contract_case).map_err(Failed::from),
        ));

        let smoke_case = case.clone();
        trials.push(Trial::test(
            format!("smoke::{}", smoke_case.name),
            move || run_smoke(&smoke_case).map_err(Failed::from),
        ));

        let live_case = case;
        trials.push(
            Trial::test(format!("live::{}", live_case.name), move || {
                let config = live_config_from_env().map_err(Failed::from)?;
                run_live(&live_case, &config)
                    .map(|_| ())
                    .map_err(Failed::from)
            })
            .with_ignored_flag(true),
        );
    }

    trials
}
