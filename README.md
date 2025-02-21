# Triton CLI

[![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![GitHub CI](https://github.com/TritonVM/triton-cli/actions/workflows/main.yml/badge.svg)](https://github.com/TritonVM/triton-cli/actions)
[![crates.io](https://img.shields.io/crates/v/triton-cli.svg)](https://crates.io/crates/triton-cli)
[![Coverage Status](https://coveralls.io/repos/github/TritonVM/triton-cli/badge.svg?branch=main)](https://coveralls.io/github/TritonVM/triton-cli?branch=main)

Command Line Interface (CLI) for the [Zero-Knowledge Virtual Machine Triton](https://triton-vm.org).
Triton CLI lets you

- execute programs written for Triton VM,
- prove the correct execution of such programs, and
- verify a claimed execution result.

You might also be interested in the [Triton TUI](https://github.com/TritonVM/triton-tui), which is
helpful when debugging Triton programs.

## Usage

### Execute a Triton program

The `run` command of Triton CLI executes a Triton program to completion, but does not generate any
proofs of correct execution. The command expects

- the program that is to be executed,
- optionally either the input to the program or a file containing the program's input, and
- optionally a file containing the program's non-determinism, Triton VM's interface for secret
  input. (To better understand non-determinism, take a look at
  the [explanation](https://docs.rs/triton-vm/0.48.0/triton_vm/#non-determinism) given in Triton.)

For example, to run a program with input `42,43,44` or with input from a file `input.txt`, use:

```sh
triton-cli run --program program.tasm --input 42,43,44
triton-cli run --program program.tasm --input-file input.txt
```

Alternatively, you can specify a file containing Triton's entire initial state. All necessary
information (the program, its input, and non-determinism) are contained in this JSON file. It's
probably easiest to get such a file programmatically, by serializing a Triton
[`VMState`](https://docs.rs/triton-vm/0.48.0/triton_vm/vm/struct.VMState.html) object.

```sh
triton-cli run --initial-state triton_state.json
```

In either case, successful execution with graceful termination will print the computed output to
stdout. If program causes Triton
to [crash](https://docs.rs/triton-vm/0.48.0/triton_vm/#crashing-triton-vm), the corresponding error
is printed to stderr.

### Prove correct execution of a Triton program

The `prove` command generates a proof of correct execution of a Triton program, as well as a summary
of what is [claimed](https://docs.rs/triton-vm/0.48.0/triton_vm/proof/struct.Claim.html). Notably,
this claim contains the input to the program, the program's output, as well as the hash digest of
the program.

Command `prove` requires the same arguments as the `run` command, and takes additional arguments
to specify the locations of the produced proof and claim files. The additional arguments default to
`triton.proof` and `triton.claim`, respectively. For example:

```sh
triton-cli prove --program program.tasm --input 42,43,44 --proof program.proof
triton-cli prove --initial-state triton_state.json --claim program.claim
```

Existing files will be overwritten.

### Verify a claimed execution result

The `verify` command checks the correctness of a claimed execution result. It requires a file
containing the claim and a file containing the proof. The default locations are `triton.claim` and
`triton.proof`, respectively. For example:

```sh
triton-cli verify
triton-cli verify --claim program.claim --proof program.proof
```

## Installation

### From [crates.io](https://crates.io/crates/triton-cli)

```sh
cargo install triton-cli
```

### Binaries

Check out the [releases page](https://github.com/TritonVM/triton-cli/releases).
