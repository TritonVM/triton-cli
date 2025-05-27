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

Triton CLI optionally prints command-dependent profiling information for each of these commands.

You might also be interested in the [Triton TUI](https://github.com/TritonVM/triton-tui), which is
helpful when debugging Triton programs.

## Installation

### From [crates.io](https://crates.io/crates/triton-cli)

```sh
cargo install triton-cli
```

### From [Github](https://github.com/TritonVM/triton-cli)

```sh
git clone https://github.com/TritonVM/triton-cli.git
cd triton-cli
cargo install --path .
```

### Binaries

Check out the [releases page](https://github.com/TritonVM/triton-cli/releases).

## Usage

### Execute a Triton Program

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
standard output (`stdout`). If the program causes Triton to
[crash](https://docs.rs/triton-vm/0.48.0/triton_vm/#crashing-triton-vm), the corresponding error
is printed to standard error (`stderr`).

### Prove Correct Execution of a Triton Program

The `prove` command generates a proof of correct execution of a Triton program, as well as a summary
of what is [claimed](https://docs.rs/triton-vm/0.48.0/triton_vm/proof/struct.Claim.html). Notably,
this claim contains the input to the program, the program's output, as well as the hash digest of
the program.

Command `prove` requires the same arguments as the `run` command, and takes additional arguments
to specify the locations of the produced proof and claim files. The additional arguments default to
`triton.proof` and `triton.claim`, respectively. For example:

```sh
triton-cli prove --program program.tasm --input 42,43,44 --proof triton.proof
triton-cli prove --initial-state triton_state.json --claim triton.claim
```

Existing files will be overwritten silently.

### Verify a Claimed Execution Result

The `verify` command checks the correctness of a claimed execution result. It requires a file
containing the claim and a file containing the proof. The default locations are `triton.claim` and
`triton.proof`, respectively. For example, the following are equivalent:

```sh
triton-cli verify
triton-cli verify --claim triton.claim --proof triton.proof
```

## Profiling

Triton CLI accepts the `--profile` flag preceding any valid command. Depending on the command, a
different kind of profile will be printed to standard out (`stdout`).

- `triton-cli --profile run` prints an execution profile. This profile contains information about
the program's subroutines and their contribution to the execution trace. This is particularly useful
when you are developing a program and want to know its performance characteristics.
- `triton-cli --profile prove` and `triton-cli --profile verify` print a performance profile. This
profile lists the steps performed by Triton VM's prover and verifier, respectively, and details the
amount of time spent overall and at each step, as well as the overall memory usage and each step's
estimated contribution.

### Identify Proving Capabilities

Proving correct execution of a program is an inherently resource intensive operation. In order to
identify the proving capabilities of your machine, you can profile the below Triton assembly program
for different inputs in the range 16 to 31 (both inclusive).

```tasm
read_io 1
  hint log2_padded_height: u32 = stack[0]

// only log₂(padded heights) in range 16..32 are supported
dup 0 push 15 lt assert error_id 0
push 32 dup 1 lt assert error_id 1

// compute number of spin-loop iterations to get to requested
// padded height
addi -15
push 2 pow
push 1000 mul
  hint num_iterations = stack[0]

// do the spin 🌀
sponge_init
call spin
halt

// BEFORE: _ num_iterations
// AFTER:  _ 0
spin:
  sponge_squeeze                // _ n [_; 10]
  write_mem 5 write_mem 4       // _ n [_; 1]

  split
  pop 2                         // _ n

  dup 0 push 0 eq skiz return
  addi -1 recurse
```

Example usage:

```sh
triton-cli --profile prove --program spin.tasm --input 16
```