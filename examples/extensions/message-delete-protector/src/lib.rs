use corrode_extension_sdk::{Invocation, Output};

fn activate(_input: Invocation) -> Output {
	Output::default()
}

corrode_extension_sdk::export!(activate);
