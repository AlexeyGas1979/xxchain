// This file is part of Substrate.

// Copyright (C) 2018-2021 Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use codec::{Encode, Decode, Joiner};
use frame_support::{
	traits::Currency,
	weights::{GetDispatchInfo, DispatchInfo, DispatchClass},
};
use sp_core::{NeverNativeValue, traits::Externalities, storage::well_known_keys};
use sp_runtime::{
	ApplyExtrinsicResult,
	transaction_validity::InvalidTransaction,
};
use frame_system::{self, EventRecord, Phase, AccountInfo};

use xxnetwork_runtime::{
	Header, Block, UncheckedExtrinsic, CheckedExtrinsic, Call, Runtime, Balances,
	System, TransactionPayment, Event,
	constants::{time::SLOT_DURATION, currency::*},
};
use node_primitives::{Balance, Hash};

use node_testing::keyring::*;

pub mod common;
use self::common::{*, sign};

/// The wasm runtime binary which hasn't undergone the compacting process.
///
/// The idea here is to pass it as the current runtime code to the executor so the executor will
/// have to execute provided wasm code instead of the native equivalent. This trick is used to
/// test code paths that differ between native and wasm versions.
pub fn bloaty_code_unwrap() -> &'static [u8] {
	xxnetwork_runtime::WASM_BINARY_BLOATY.expect("Development wasm binary is not available. \
											 Testing is only supported with the flag disabled.")
}

/// Default transfer fee. This will use the same logic that is implemented in transaction-payment module.
///
/// Note that reads the multiplier from storage directly, hence to get the fee of `extrinsic`
/// at block `n`, it must be called prior to executing block `n` to do the calculation with the
/// correct multiplier.
fn transfer_fee<E: Encode>(extrinsic: &E) -> Balance {
	TransactionPayment::compute_fee(
		extrinsic.encode().len() as u32,
		&default_transfer_call().get_dispatch_info(),
		0,
	)
}

fn xt() -> UncheckedExtrinsic {
	sign(CheckedExtrinsic {
		signed: Some((alice(), signed_extra(0, 0))),
		function: Call::Balances(default_transfer_call()),
	})
}

fn set_heap_pages<E: Externalities>(ext: &mut E, heap_pages: u64) {
	ext.place_storage(well_known_keys::HEAP_PAGES.to_vec(), Some(heap_pages.encode()));
}

fn changes_trie_block() -> (Vec<u8>, Hash) {
	let time = 42 * 1000;
	construct_block(
		&mut new_test_ext(compact_code_unwrap(), true),
		1,
		GENESIS_HASH.into(),
		vec![
			CheckedExtrinsic {
				signed: None,
				function: Call::Timestamp(pallet_timestamp::Call::set(time)),
			},
			CheckedExtrinsic {
				signed: Some((alice(), signed_extra(0, 0))),
				function: Call::Balances(pallet_balances::Call::transfer(bob().into(), 69 * UNITS)),
			},
		],
		(time / SLOT_DURATION).into(),
	)
}

/// block 1 and 2 must be created together to ensure transactions are only signed once (since they
/// are not guaranteed to be deterministic) and to ensure that the correct state is propagated
/// from block1's execution to block2 to derive the correct storage_root.
fn blocks() -> ((Vec<u8>, Hash), (Vec<u8>, Hash)) {
	let mut t = new_test_ext(compact_code_unwrap(), false);
	let time1 = 42 * 1000;
	let block1 = construct_block(
		&mut t,
		1,
		GENESIS_HASH.into(),
		vec![
			CheckedExtrinsic {
				signed: None,
				function: Call::Timestamp(pallet_timestamp::Call::set(time1)),
			},
			CheckedExtrinsic {
				signed: Some((alice(), signed_extra(0, 0))),
				function: Call::Balances(pallet_balances::Call::transfer(bob().into(), 69 * UNITS)),
			},
		],
		(time1 / SLOT_DURATION).into(),
	);
	let time2 = 52 * 1000;
	let block2 = construct_block(
		&mut t,
		2,
		block1.1.clone(),
		vec![
			CheckedExtrinsic {
				signed: None,
				function: Call::Timestamp(pallet_timestamp::Call::set(time2)),
			},
			CheckedExtrinsic {
				signed: Some((bob(), signed_extra(0, 0))),
				function: Call::Balances(pallet_balances::Call::transfer(alice().into(), 5 * UNITS)),
			},
			CheckedExtrinsic {
				signed: Some((alice(), signed_extra(1, 0))),
				function: Call::Balances(pallet_balances::Call::transfer(bob().into(), 15 * UNITS)),
			}
		],
		(time2 / SLOT_DURATION).into(),
	);

	// session change => consensus authorities change => authorities change digest item appears
	let digest = Header::decode(&mut &block2.0[..]).unwrap().digest;
	assert_eq!(digest.logs().len(), 1 /* Just babe slot */);

	(block1, block2)
}

fn block_with_size(time: u64, nonce: u32, size: usize) -> (Vec<u8>, Hash) {
	construct_block(
		&mut new_test_ext(compact_code_unwrap(), false),
		1,
		GENESIS_HASH.into(),
		vec![
			CheckedExtrinsic {
				signed: None,
				function: Call::Timestamp(pallet_timestamp::Call::set(time * 1000)),
			},
			CheckedExtrinsic {
				signed: Some((alice(), signed_extra(nonce, 0))),
				function: Call::System(frame_system::Call::remark(vec![0; size])),
			}
		],
		(time * 1000 / SLOT_DURATION).into(),
	)
}

#[test]
fn panic_execution_with_foreign_code_gives_error() {
	let mut t = new_test_ext(bloaty_code_unwrap(), false);
	t.insert(
		<frame_system::Account<Runtime>>::hashed_key_for(alice()),
		(69u128, 0u32, 0u128, 0u128, 0u128).encode()
	);
	t.insert(<pallet_balances::TotalIssuance<Runtime>>::hashed_key().to_vec(), 69_u128.encode());
	t.insert(<frame_system::BlockHash<Runtime>>::hashed_key_for(0), vec![0u8; 32]);

	let r = executor_call::<NeverNativeValue, fn() -> _>(
		&mut t,
		"Core_initialize_block",
		&vec![].and(&from_block_number(1u32)),
		true,
		None,
	).0;
	assert!(r.is_ok());
	let v = executor_call::<NeverNativeValue, fn() -> _>(
		&mut t,
		"BlockBuilder_apply_extrinsic",
		&vec![].and(&xt()),
		true,
		None,
	).0.unwrap();
	let r = ApplyExtrinsicResult::decode(&mut &v.as_encoded()[..]).unwrap();
	assert_eq!(r, Err(InvalidTransaction::Payment.into()));
}

#[test]
fn bad_extrinsic_with_native_equivalent_code_gives_error() {
	let mut t = new_test_ext(compact_code_unwrap(), false);
	t.insert(
		<frame_system::Account<Runtime>>::hashed_key_for(alice()),
		(0u32, 0u32, 0u32, 69u128, 0u128, 0u128, 0u128).encode()
	);
	t.insert(<pallet_balances::TotalIssuance<Runtime>>::hashed_key().to_vec(), 69_u128.encode());
	t.insert(<frame_system::BlockHash<Runtime>>::hashed_key_for(0), vec![0u8; 32]);

	let r = executor_call::<NeverNativeValue, fn() -> _>(
		&mut t,
		"Core_initialize_block",
		&vec![].and(&from_block_number(1u32)),
		true,
		None,
	).0;
	assert!(r.is_ok());
	let v = executor_call::<NeverNativeValue, fn() -> _>(
		&mut t,
		"BlockBuilder_apply_extrinsic",
		&vec![].and(&xt()),
		true,
		None,
	).0.unwrap();
	let r = ApplyExtrinsicResult::decode(&mut &v.as_encoded()[..]).unwrap();
	assert_eq!(r, Err(InvalidTransaction::Payment.into()));
}

#[test]
fn successful_execution_with_native_equivalent_code_gives_ok() {
	let mut t = new_test_ext(compact_code_unwrap(), false);
	t.insert(
		<frame_system::Account<Runtime>>::hashed_key_for(alice()),
		AccountInfo::<<Runtime as frame_system::Config>::Index, _> {
			data: (111 * UNITS, 0u128, 0u128, 0u128),
			.. Default::default()
		}.encode(),
	);
	t.insert(
		<frame_system::Account<Runtime>>::hashed_key_for(bob()),
		AccountInfo::<<Runtime as frame_system::Config>::Index, _> {
			data: (0 * UNITS, 0u128, 0u128, 0u128),
			.. Default::default()
		}.encode(),
	);
	t.insert(
		<pallet_balances::TotalIssuance<Runtime>>::hashed_key().to_vec(),
		(111 * UNITS).encode()
	);
	t.insert(<frame_system::BlockHash<Runtime>>::hashed_key_for(0), vec![0u8; 32]);

	let r = executor_call::<NeverNativeValue, fn() -> _>(
		&mut t,
		"Core_initialize_block",
		&vec![].and(&from_block_number(1u32)),
		true,
		None,
	).0;
	assert!(r.is_ok());

	let fees = t.execute_with(|| transfer_fee(&xt()));

	let r = executor_call::<NeverNativeValue, fn() -> _>(
		&mut t,
		"BlockBuilder_apply_extrinsic",
		&vec![].and(&xt()),
		true,
		None,
	).0;
	assert!(r.is_ok());

	t.execute_with(|| {
		assert_eq!(Balances::total_balance(&alice()), 42 * UNITS - fees);
		assert_eq!(Balances::total_balance(&bob()), 69 * UNITS);
	});
}

#[test]
fn successful_execution_with_foreign_code_gives_ok() {
	let mut t = new_test_ext(bloaty_code_unwrap(), false);
	t.insert(
		<frame_system::Account<Runtime>>::hashed_key_for(alice()),
		AccountInfo::<<Runtime as frame_system::Config>::Index, _> {
			data: (111 * UNITS, 0u128, 0u128, 0u128),
			.. Default::default()
		}.encode(),
	);
	t.insert(
		<frame_system::Account<Runtime>>::hashed_key_for(bob()),
		AccountInfo::<<Runtime as frame_system::Config>::Index, _> {
			data: (0 * UNITS, 0u128, 0u128, 0u128),
			.. Default::default()
		}.encode(),
	);
	t.insert(
		<pallet_balances::TotalIssuance<Runtime>>::hashed_key().to_vec(),
		(111 * UNITS).encode()
	);
	t.insert(<frame_system::BlockHash<Runtime>>::hashed_key_for(0), vec![0u8; 32]);

	let r = executor_call::<NeverNativeValue, fn() -> _>(
		&mut t,
		"Core_initialize_block",
		&vec![].and(&from_block_number(1u32)),
		true,
		None,
	).0;
	assert!(r.is_ok());

	let fees = t.execute_with(|| transfer_fee(&xt()));

	let r = executor_call::<NeverNativeValue, fn() -> _>(
		&mut t,
		"BlockBuilder_apply_extrinsic",
		&vec![].and(&xt()),
		true,
		None,
	).0;
	assert!(r.is_ok());

	t.execute_with(|| {
		assert_eq!(Balances::total_balance(&alice()), 42 * UNITS - fees);
		assert_eq!(Balances::total_balance(&bob()), 69 * UNITS);
	});
}

#[test]
fn full_native_block_import_works() {
	let mut t = new_test_ext(compact_code_unwrap(), false);

	let (block1, block2) = blocks();

	let mut alice_last_known_balance: Balance = Default::default();
	let mut fees = t.execute_with(|| transfer_fee(&xt()));

	let transfer_weight = default_transfer_call().get_dispatch_info().weight;
	let timestamp_weight = pallet_timestamp::Call::set::<Runtime>(Default::default()).get_dispatch_info().weight;

	executor_call::<NeverNativeValue, fn() -> _>(
		&mut t,
		"Core_execute_block",
		&block1.0,
		true,
		None,
	).0.unwrap();

	t.execute_with(|| {
		assert_eq!(Balances::total_balance(&alice()), 42 * UNITS - fees);
		assert_eq!(Balances::total_balance(&bob()), 169 * UNITS);
		alice_last_known_balance = Balances::total_balance(&alice());
		let events = vec![
			EventRecord {
				phase: Phase::ApplyExtrinsic(0),
				event: Event::System(frame_system::Event::ExtrinsicSuccess(
					DispatchInfo { weight: timestamp_weight, class: DispatchClass::Mandatory, ..Default::default() }
				)),
				topics: vec![],
			},
			EventRecord {
				phase: Phase::ApplyExtrinsic(1),
				event: Event::Balances(pallet_balances::Event::Transfer(
					alice().into(),
					bob().into(),
					69 * UNITS,
				)),
				topics: vec![],
			},
			// 80% of fees to Treasury
			EventRecord {
				phase: Phase::ApplyExtrinsic(1),
				event: Event::Treasury(pallet_treasury::Event::Deposit(fees * 8 / 10)),
				topics: vec![],
			},
			// 20% of fees to Block Author
			EventRecord {
				phase: Phase::ApplyExtrinsic(1),
				event: Event::Balances(pallet_balances::Event::Deposit(
					Default::default(),
					fees * 2 / 10 + 1, // Rounding error
				)),
				topics: vec![],
			},
			EventRecord {
				phase: Phase::ApplyExtrinsic(1),
				event: Event::System(frame_system::Event::ExtrinsicSuccess(
					DispatchInfo { weight: transfer_weight, ..Default::default() }
				)),
				topics: vec![],
			},
		];
		assert_eq!(System::events(), events);
	});

	fees = t.execute_with(|| transfer_fee(&xt()));

	executor_call::<NeverNativeValue, fn() -> _>(
		&mut t,
		"Core_execute_block",
		&block2.0,
		true,
		None,
	).0.unwrap();

	t.execute_with(|| {
		assert_eq!(
			Balances::total_balance(&alice()),
			alice_last_known_balance - 10 * UNITS - fees,
		);
		assert_eq!(
			Balances::total_balance(&bob()),
			179 * UNITS - fees,
		);
		let events = vec![
			EventRecord {
				phase: Phase::ApplyExtrinsic(0),
				event: Event::System(frame_system::Event::ExtrinsicSuccess(
					DispatchInfo { weight: timestamp_weight, class: DispatchClass::Mandatory, ..Default::default() }
				)),
				topics: vec![],
			},
			EventRecord {
				phase: Phase::ApplyExtrinsic(1),
				event: Event::Balances(
					pallet_balances::Event::Transfer(
						bob().into(),
						alice().into(),
						5 * UNITS,
					)
				),
				topics: vec![],
			},
			// 80% of fees to Treasury
			EventRecord {
				phase: Phase::ApplyExtrinsic(1),
				event: Event::Treasury(pallet_treasury::Event::Deposit(fees * 8 / 10)),
				topics: vec![],
			},
			// 20% of fees to Block Author
			EventRecord {
				phase: Phase::ApplyExtrinsic(1),
				event: Event::Balances(pallet_balances::Event::Deposit(
					Default::default(),
					fees * 2 / 10 + 1, // Rounding error
				)),
				topics: vec![],
			},
			EventRecord {
				phase: Phase::ApplyExtrinsic(1),
				event: Event::System(frame_system::Event::ExtrinsicSuccess(
					DispatchInfo { weight: transfer_weight, ..Default::default() }
				)),
				topics: vec![],
			},
			EventRecord {
				phase: Phase::ApplyExtrinsic(2),
				event: Event::Balances(
					pallet_balances::Event::Transfer(
						alice().into(),
						bob().into(),
						15 * UNITS,
					)
				),
				topics: vec![],
			},
			// 80% of fees to Treasury
			EventRecord {
				phase: Phase::ApplyExtrinsic(2),
				event: Event::Treasury(pallet_treasury::Event::Deposit(fees * 8 / 10)),
				topics: vec![],
			},
			// 20% of fees to Block Author
			EventRecord {
				phase: Phase::ApplyExtrinsic(2),
				event: Event::Balances(pallet_balances::Event::Deposit(
					Default::default(),
					fees * 2 / 10 + 1, // Rounding error
				)),
				topics: vec![],
			},
			EventRecord {
				phase: Phase::ApplyExtrinsic(2),
				event: Event::System(frame_system::Event::ExtrinsicSuccess(
					DispatchInfo { weight: transfer_weight, ..Default::default() }
				)),
				topics: vec![],
			},
		];
		assert_eq!(System::events(), events);
	});
}

#[test]
fn full_wasm_block_import_works() {
	let mut t = new_test_ext(compact_code_unwrap(), false);

	let (block1, block2) = blocks();

	let mut alice_last_known_balance: Balance = Default::default();
	let mut fees = t.execute_with(|| transfer_fee(&xt()));

	executor_call::<NeverNativeValue, fn() -> _>(
		&mut t,
		"Core_execute_block",
		&block1.0,
		false,
		None,
	).0.unwrap();

	t.execute_with(|| {
		assert_eq!(Balances::total_balance(&alice()), 42 * UNITS - fees);
		assert_eq!(Balances::total_balance(&bob()), 169 * UNITS);
		alice_last_known_balance = Balances::total_balance(&alice());
	});

	fees = t.execute_with(|| transfer_fee(&xt()));

	executor_call::<NeverNativeValue, fn() -> _>(
		&mut t,
		"Core_execute_block",
		&block2.0,
		false,
		None,
	).0.unwrap();

	t.execute_with(|| {
		assert_eq!(
			Balances::total_balance(&alice()),
			alice_last_known_balance - 10 * UNITS - fees,
		);
		assert_eq!(
			Balances::total_balance(&bob()),
			179 * UNITS - 1 * fees,
		);
	});
}

#[test]
fn wasm_big_block_import_fails() {
	let mut t = new_test_ext(compact_code_unwrap(), false);

	set_heap_pages(&mut t.ext(), 4);

	let result = executor_call::<NeverNativeValue, fn() -> _>(
		&mut t,
		"Core_execute_block",
		&block_with_size(42, 0, 120_000).0,
		false,
		None,
	).0;
	assert!(result.is_err()); // Err(Wasmi(Trap(Trap { kind: Host(AllocatorOutOfSpace) })))
}

#[test]
fn native_big_block_import_succeeds() {
	let mut t = new_test_ext(compact_code_unwrap(), false);

	executor_call::<NeverNativeValue, fn() -> _>(
		&mut t,
		"Core_execute_block",
		&block_with_size(42, 0, 120_000).0,
		true,
		None,
	).0.unwrap();
}

#[test]
fn native_big_block_import_fails_on_fallback() {
	let mut t = new_test_ext(compact_code_unwrap(), false);

	// We set the heap pages to 8 because we know that should give an OOM in WASM with the given
	// block.
	set_heap_pages(&mut t.ext(), 8);

	assert!(
		executor_call::<NeverNativeValue, fn() -> _>(
			&mut t,
			"Core_execute_block",
			&block_with_size(42, 0, 120_000).0,
			false,
			None,
		).0.is_err()
	);
}

#[test]
fn panic_execution_gives_error() {
	let mut t = new_test_ext(bloaty_code_unwrap(), false);
	t.insert(
		<frame_system::Account<Runtime>>::hashed_key_for(alice()),
		AccountInfo::<<Runtime as frame_system::Config>::Index, _> {
			data: (0 * UNITS, 0u128, 0u128, 0u128),
			.. Default::default()
		}.encode(),
	);
	t.insert(<pallet_balances::TotalIssuance<Runtime>>::hashed_key().to_vec(), 0_u128.encode());
	t.insert(<frame_system::BlockHash<Runtime>>::hashed_key_for(0), vec![0u8; 32]);

	let r = executor_call::<NeverNativeValue, fn() -> _>(
		&mut t,
		"Core_initialize_block",
		&vec![].and(&from_block_number(1u32)),
		false,
		None,
	).0;
	assert!(r.is_ok());
	let r = executor_call::<NeverNativeValue, fn() -> _>(
		&mut t,
		"BlockBuilder_apply_extrinsic",
		&vec![].and(&xt()),
		false,
		None,
	).0.unwrap().into_encoded();
	let r = ApplyExtrinsicResult::decode(&mut &r[..]).unwrap();
	assert_eq!(r, Err(InvalidTransaction::Payment.into()));
}

#[test]
fn successful_execution_gives_ok() {
	let mut t = new_test_ext(compact_code_unwrap(), false);
	t.insert(
		<frame_system::Account<Runtime>>::hashed_key_for(alice()),
		AccountInfo::<<Runtime as frame_system::Config>::Index, _> {
			data: (111 * UNITS, 0u128, 0u128, 0u128),
			.. Default::default()
		}.encode(),
	);
	t.insert(
		<frame_system::Account<Runtime>>::hashed_key_for(bob()),
		AccountInfo::<<Runtime as frame_system::Config>::Index, _> {
			data: (0 * UNITS, 0u128, 0u128, 0u128),
			.. Default::default()
		}.encode(),
	);
	t.insert(
		<pallet_balances::TotalIssuance<Runtime>>::hashed_key().to_vec(),
		(111 * UNITS).encode()
	);
	t.insert(<frame_system::BlockHash<Runtime>>::hashed_key_for(0), vec![0u8; 32]);

	let r = executor_call::<NeverNativeValue, fn() -> _>(
		&mut t,
		"Core_initialize_block",
		&vec![].and(&from_block_number(1u32)),
		false,
		None,
	).0;
	assert!(r.is_ok());
	t.execute_with(|| {
		assert_eq!(Balances::total_balance(&alice()), 111 * UNITS);
	});

	let fees = t.execute_with(|| transfer_fee(&xt()));

	let r = executor_call::<NeverNativeValue, fn() -> _>(
		&mut t,
		"BlockBuilder_apply_extrinsic",
		&vec![].and(&xt()),
		false,
		None,
	).0.unwrap().into_encoded();
	ApplyExtrinsicResult::decode(&mut &r[..])
		.unwrap()
		.expect("Extrinsic could not be applied")
		.expect("Extrinsic failed");

	t.execute_with(|| {
		assert_eq!(Balances::total_balance(&alice()), 42 * UNITS - fees);
		assert_eq!(Balances::total_balance(&bob()), 69 * UNITS);
	});
}

#[test]
fn full_native_block_import_works_with_changes_trie() {
	let block1 = changes_trie_block();
	let block_data = block1.0;
	let block = Block::decode(&mut &block_data[..]).unwrap();

	let mut t = new_test_ext(compact_code_unwrap(), true);
	executor_call::<NeverNativeValue, fn() -> _>(
		&mut t,
		"Core_execute_block",
		&block.encode(),
		true,
		None,
	).0.unwrap();

	assert!(t.ext().storage_changes_root(&GENESIS_HASH).unwrap().is_some());
}

#[test]
fn full_wasm_block_import_works_with_changes_trie() {
	let block1 = changes_trie_block();

	let mut t = new_test_ext(compact_code_unwrap(), true);
	executor_call::<NeverNativeValue, fn() -> _>(
		&mut t,
		"Core_execute_block",
		&block1.0,
		false,
		None,
	).0.unwrap();

	assert!(t.ext().storage_changes_root(&GENESIS_HASH).unwrap().is_some());
}

#[test]
fn should_import_block_with_test_client() {
	use node_testing::client::{
		ClientBlockImportExt, TestClientBuilderExt, TestClientBuilder,
		sp_consensus::BlockOrigin,
	};

	let mut client = TestClientBuilder::new().build();
	let block1 = changes_trie_block();
	let block_data = block1.0;
	let block = node_primitives::Block::decode(&mut &block_data[..]).unwrap();

	futures::executor::block_on(client.import(BlockOrigin::Own, block)).unwrap();
}
