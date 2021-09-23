// This file is part of Substrate.

// Copyright (C) 2021 Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Autogenerated weights for swap
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 4.0.0-dev
//! DATE: 2021-09-13, STEPS: `50`, REPEAT: 20, LOW RANGE: `[]`, HIGH RANGE: `[]`
//! EXECUTION: Some(Wasm), WASM-EXECUTION: Compiled, CHAIN: Some("xxnetwork-dev"), DB CACHE: 128

// Executed Command:
// target/release/xxnetwork-chain
// benchmark
// --chain=xxnetwork-dev
// --steps=50
// --repeat=20
// --pallet=swap
// --extrinsic=*
// --execution=wasm
// --wasm-execution=compiled
// --heap-pages=4096
// --output=./swap/src/weights.rs
// --template=./scripts/frame-weight-template.hbs


#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use sp_std::marker::PhantomData;

/// Weight functions needed for swap.
pub trait WeightInfo {
	fn transfer_native() -> Weight;
	fn transfer() -> Weight;
	fn set_swap_fee() -> Weight;
	fn set_fee_destination() -> Weight;
}

/// Weights for swap using the Substrate node and recommended hardware.
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	// Storage: ChainBridge ChainNonces (r:1 w:1)
	// Storage: Swap FeeDestination (r:1 w:0)
	// Storage: System Account (r:3 w:3)
	// Storage: Swap SwapFee (r:1 w:0)
	fn transfer_native() -> Weight {
		(112_387_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(6 as Weight))
			.saturating_add(T::DbWeight::get().writes(4 as Weight))
	}
	// Storage: System Account (r:2 w:2)
	fn transfer() -> Weight {
		(68_909_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(2 as Weight))
			.saturating_add(T::DbWeight::get().writes(2 as Weight))
	}
	// Storage: Swap SwapFee (r:0 w:1)
	fn set_swap_fee() -> Weight {
		(19_255_000 as Weight)
			.saturating_add(T::DbWeight::get().writes(1 as Weight))
	}
	// Storage: Swap FeeDestination (r:0 w:1)
	fn set_fee_destination() -> Weight {
		(19_658_000 as Weight)
			.saturating_add(T::DbWeight::get().writes(1 as Weight))
	}
}

// For backwards compatibility and tests
impl WeightInfo for () {
	// Storage: ChainBridge ChainNonces (r:1 w:1)
	// Storage: Swap FeeDestination (r:1 w:0)
	// Storage: System Account (r:3 w:3)
	// Storage: Swap SwapFee (r:1 w:0)
	fn transfer_native() -> Weight {
		(112_387_000 as Weight)
			.saturating_add(RocksDbWeight::get().reads(6 as Weight))
			.saturating_add(RocksDbWeight::get().writes(4 as Weight))
	}
	// Storage: System Account (r:2 w:2)
	fn transfer() -> Weight {
		(68_909_000 as Weight)
			.saturating_add(RocksDbWeight::get().reads(2 as Weight))
			.saturating_add(RocksDbWeight::get().writes(2 as Weight))
	}
	// Storage: Swap SwapFee (r:0 w:1)
	fn set_swap_fee() -> Weight {
		(19_255_000 as Weight)
			.saturating_add(RocksDbWeight::get().writes(1 as Weight))
	}
	// Storage: Swap FeeDestination (r:0 w:1)
	fn set_fee_destination() -> Weight {
		(19_658_000 as Weight)
			.saturating_add(RocksDbWeight::get().writes(1 as Weight))
	}
}
