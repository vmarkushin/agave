# Solana Virtual Machine specification

# Introduction

Several components of the Solana Validator are involved in processing
a transaction (or a batch of transactions).  Collectively, the
components responsible for transaction execution are designated as
Solana Virtual Machine (SVM). SVM packaged as a stand-alone library
can be used in applications outside the Solana Validator.

This document represents the SVM specification. It covers the API
of using SVM in projects unrelated to Solana Validator and the
internal workings of the SVM, including the descriptions of the inner
data flow, data structures, and algorithms involved in the execution
of transactions. The document’s target audience includes both external
users and the developers of the SVM.

## Use cases

We envision the following applications for SVM

- **Transaction execution in Solana Validator**

    This is the primary use case for the SVM. It remains a major
    component of the Agave Validator, but with clear interface and
    isolated from dependencies on other components.

    The SVM is currently viewed as realizing two stages of the
    Transaction Engine Execution pipeline as described in Solana
    Architecture documentation
    [https://docs.solana.com/validator/runtime#execution](https://docs.solana.com/validator/runtime#execution),
    namely ‘load accounts’ and ‘execute’ stages.

- **SVM Rollups**

    Rollups that need to execute a block but don’t need the other
    components of the validator can benefit from SVM, as it can reduce
    hardware requirements and decentralize the network. This is
    especially useful for Ephemeral Rollups since the cost of compute
    will be higher as a new rollup is created for every user session
    in applications like gaming.

- **SVM Fraud Proofs for Diet Clients**

    A succinct proof of an invalid state transition by the supermajority (SIMD-65)

- **Validator Sidecar for JSON-RPC**

    The RPC needs to be separated from the validator.
    `simulateTransaction` requires replaying the transactions and
    accessing necessary account data.

- **SVM-based Avalanche subnet**

    The SVM would need to be isolated to run within a subnet since the
    consensus and networking functionality would rely on Avalanche
    modules.

- **Modified SVM (SVM+)**

    An SVM type with all the current functionality and extended
    instructions for custom use cases. This would form a superset of
    the current SVM.

# System Context

In this section, SVM is represented as a single entity. We describe its
interfaces to the parts of the Solana Validator external to SVM.

In the context of Solana Validator, the main entity external to SVM is
bank. It creates an SVM, submits transactions for execution and
receives results of transaction execution from SVM.

![context diagram](/svm/doc/diagrams/context.svg "System Context")

## Interfaces

In this section, we describe the API of using the SVM both in Solana
Validator and in third-party applications.

The interface to SVM is represented by the
`transaction_processor::TransactionBatchProcessor` struct.  To create
a `TransactionBatchProcessor` object the client need to specify the
`slot`, `epoch`, `epoch_schedule`, `fee_structure`, `runtime_config`,
and `program_cache`.

The main entry point to the SVM is the method
`load_and_execute_sanitized_transactions`. In addition
`TransactionBatchProcessor` provides utility methods
    - `load_program_with_pubkey`, used in Bank to load program with a
      specific pubkey from loaded programs cache, and update the program's
      access slot as a side-effect;
    - `program_modification_slot`, used in Bank to find the slot in
      which the program was most recently modified.

The method `load_and_execute_sanitized_transactions` takes the
following arguments
    - `callbacks` is a `TransactionProcessingCallback` trait instance
      that enables access to data available from accounts-db and from
      Bank,
    - `sanitized_txs` a slice of `SanitizedTransaction`
      - `SanitizedTransaction` contains
        - `SanitizedMessage` is an enum with two kinds of messages
          - `LegacyMessage` and `LoadedMessage`
            Both `LegacyMessage` and `LoadedMessage` consist of
            - `MessageHeader`
            - vector of `Pubkey` of accounts used in the transaction
            - `Hash` of recent block
            - vector of `CompiledInstruction`
            In addition `LoadedMessage` contains a vector of
            `MessageAddressTableLookup` -- list of address table lookups to
            load additional accounts for this transaction.
        - a Hash of the message
        - a boolean flag `is_simple_vote_tx` -- explain
        - a vector of `Signature`  -- explain which signatures are in this vector
    - `check_results` is a mutable slice of `TransactionCheckResult`
    - `error_counters` is a mutable reference to `TransactionErrorMetrics`
    - `recording_config` is a value of `ExecutionRecordingConfig` configuration parameters
    - `timings` is a mutable reference to `ExecuteTimings`
    - `account_overrides` is an optional reference to `AccountOverrides`
    - `builtin_programs` is an iterator of `Pubkey` that represents builtin programs
    - `log_messages_bytes_limit` is an optional `usize` limit on the size of log messages in bytes
    - `limit_to_load_programs` is a boolean flag that instruct the function to only load the
      programs and do not execute the transactions.

The method returns a value of
`LoadAndExecuteSanitizedTransactionsOutput` which consists of two
vectors
    - a vector of `TransactionLoadResult`, and
    - a vector of `TransactionExecutionResult`.

An integration test `svm_integration` contains an example of
instantiating `TransactionBatchProcessor` and calling its method
`load_and_execute_sanitized_transactions`.

# Functional Model

In this section, we describe the functionality (logic) of the SVM in
terms of its components, relationships among components, and their
interactions.

On a high level the control flow of SVM consists of loading program
accounts, checking and verifying the loaded accounts, creating
invocation context and invoking RBPF on programs implementing the
instructions of a transaction. The SVM needs to have access to an account
database, and a sysvar cache via traits implemented for the corresponding
objects passed to it. The results of transaction execution are
consumed by bank in Solana Validator use case. However, bank structure
should not be part of the SVM.

In bank context `load_and_execute_sanitized_transactions` is called from
`simulate_transaction` where a single transaction is executed, and
from `load_execute_and_commit_transactions` which receives a batch of
transactions from its caller.

Multiple results of `load_and_execute_sanitized_transactions` are aggregated in
the struct `LoadAndExecuteSanitizedTransactionsOutput`
 - `LoadAndExecuteSanitizedTransactionsOutput` contains
  - vector of `TransactionLoadResult`
  - vector of `TransactionExecutionResult`

Steps of `load_and_execute_sanitized_transactions`

1. Steps of preparation for execution
   - filter executable program accounts and build program accounts map (explain)
   - add builtin programs to program accounts map
   - replenish program cache using the program accounts map (explain)

2. Load accounts (call to `load_accounts` function)
   - For each `SanitizedTransaction` and `TransactionCheckResult`, we:
        - Calculate the number of signatures in transaction and its cost.
        - Call `load_transaction_accounts`
            - The function is interwined with the struct `CompiledInstruction`
            - Load accounts from accounts DB
            - Extract data from accounts
            - Verify if we've reached the maximum account data size
            - Validate the fee payer and the loaded accounts
            - Validate the programs accounts that have been loaded and checks if they are builtin programs.
            - Return `struct LoadedTransaction` containing the accounts (pubkey and data),
              indices to the excutabe accounts in `TransactionContext` (or `InstructionContext`),
              the transaction rent, and the `struct RentDebit`.
            - Generate a `NonceFull` struct (holds fee subtracted nonce info) when possible, `None` otherwise.
    - Returns `TransactionLoadedResult`, a tuple containing the `LoadTransaction` we obtained from `loaded_transaction_accounts`,
      and a `Option<NonceFull>`.

3. Execute each loaded transactions
   1. Compute the sum of transaction accounts' balances. This sum is
      invariant in the transaction execution.
   2. Obtain rent state of each account before the transaction
      execution. This is later used in verifying the account state
      changes (step #7).
   3. Create a new log_collector.  `LogCollector` is defined in
      solana-program-runtime crate.
   4. Obtain last blockhash and lamports per signature. This
      information is read from blockhash_queue maintained in Bank. The
      information is taken in parameters to
      `MessageProcessor::process_message`.
   5. Make two local variables that will be used as output parameters
      of `MessageProcessor::process_message`. One will contain the
      number of executed units (the number of compute unites consumed
      in the transaction). Another is a container of `LoadedProgramsForTxBatch`.
      The latter is initialized with the slot, and
      the clone of environments of `programs_loaded_for_tx_batch`
         - `programs_loaded_for_tx_batch` contains a reference to all the `LoadedProgram`s
            necessary for the transaction. It maintains an `Arc` to the programs in the global
            `LoadedPrograms` data structure.
      6. Call `MessageProcessor::process_message` to execute the
      transaction. `MessageProcessor` is contained in
      solana-program-runtime crate. The result of processing message
      is either `ProcessedMessageInfo` which is an i64 wrapped in a
      struct meaning the change in accounts data length, or a
      `TransactionError`, if any of instructions failed to execute
      correctly.
   7. Verify transaction accounts' `RentState` changes (`verify_changes` function)
      - If the account `RentState` pre-transaction processing is rent exempt or unitiliazed, the verification will pass.
      - If the account `RentState` pre-transaction is rent paying:
         - A transition to a state uninitialized or rent exempt post-transaction is not allowed.
         - If its size has changed or its balance has increased, it cannot remain rent paying.
   8. Extract log messages.
   9. Extract inner instructions (`Vec<Vec<InnerInstruction>>`).
   10. Extract `ExecutionRecord` components from transaction context.
   11. Check balances of accounts to match the sum of balances before
       transaction execution.
   12. Update loaded transaction accounts to new accounts.
   13. Extract changes in accounts data sizes
   14. Extract return data
   15. Return `TransactionExecutionResult` with wrapping the extracted
       information in `TransactionExecutionDetails`.

4. Prepare the results of loading and executing transactions.

   This includes the following steps for each transactions
   1. Dump flattened result to info log for an account whose pubkey is
      in the transaction's debug keys.
   2. Collect logs of the transaction execution for each executed
      transaction, unless Bank's `transaction_log_collector_config` is
      set to `None`.
   3. Finally, increment various statistical counters, and update
      timings passed as a mutable reference to
      `load_and_execute_transactions` in arguments. The counters are
      packed in the struct `LoadAndExecuteTransactionsOutput`.