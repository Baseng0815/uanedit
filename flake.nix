{
  description = "uanedit — an OPC UA nodeset editor";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-26.05";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      # Keep a single nixpkgs in the lock; rust-overlay only uses its own for checks.
      inputs.nixpkgs.follows = "nixpkgs";
    };
    # flake-utils has no nixpkgs input to follow — it takes `systems` instead.
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      flake-utils,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          config.allowUnfree = true;
          overlays = [ (import rust-overlay) ];
        };

        # wasm32 is not optional here: `dx serve` builds the client half of the
        # fullstack app for wasm32-unknown-unknown and the server half natively.
        rustToolchain = pkgs.rust-bin.selectLatestNightlyWith (
          toolchain:
          toolchain.default.override {
            extensions = [
              "rust-src"
              "rust-analyzer"
              "clippy"
              "rustfmt"
            ];
            targets = [ "wasm32-unknown-unknown" ];
          }
        );

        # `dx`'s nixpkgs wrapper appends wasm-bindgen-cli 0.2.118 to PATH, but
        # wasm-bindgen aborts unless the CLI's version equals the crate version
        # Cargo.lock pins. The shell carries its own, ahead of the wrapper's.
        wasmBindgenVersion = "0.2.127";

        wasmBindgenCli =
          let
            cli = pkgs.buildWasmBindgenCli rec {
              src = pkgs.fetchCrate {
                pname = "wasm-bindgen-cli";
                version = wasmBindgenVersion;
                hash = "sha256-di+qBAdd7pENLiIB9CoZoab+W5xeDoByMREcCGTSzWo=";
              };
              cargoDeps = pkgs.rustPlatform.fetchCargoVendor {
                inherit src;
                inherit (src) pname version;
                hash = "sha256-FTv2GZIAQs0ePdIZXIXil7JbZ6kIT05VG6vqC1qNFxQ=";
              };
            };
            lines = pkgs.lib.splitString "\n" (builtins.readFile ./Cargo.lock);
            index = pkgs.lib.lists.findFirstIndex (l: l == ''name = "wasm-bindgen"'') null lines;
            locked =
              if index == null then
                wasmBindgenVersion
              else
                pkgs.lib.removeSuffix ''"'' (
                  pkgs.lib.removePrefix ''version = "'' (builtins.elemAt lines (index + 1))
                );
          in
          pkgs.lib.warnIf (locked != wasmBindgenVersion)
            "flake.nix pins wasm-bindgen-cli ${wasmBindgenVersion}, Cargo.lock pins wasm-bindgen ${locked} — bump wasmBindgenVersion and both hashes."
            cli;

        # Single knob for the shell's LLVM/clang version (stdenv, tools).
        llvmPackages = pkgs.llvmPackages;

        # `dioxus-code`'s tree-sitter grammars are C, compiled for
        # wasm32-unknown-unknown against the sysroot `arborium-sysroot` ships.
        # Three of nixpkgs's default hardening flags do not survive that
        # target: two are x86 codegen options clang rejects outright, and the
        # stack protector emits `__stack_chk_*` calls the sysroot has no
        # definitions for. Subtracting keeps the rest on, and tracks nixpkgs
        # rather than pinning a list that would drift.
        wasmHardening = pkgs.lib.subtractLists [
          "stackclashprotection"
          "stackprotector"
          "zerocallusedregs"
        ] llvmPackages.stdenv.cc.defaultHardeningFlags;
      in
      {
        devShells.default = (pkgs.mkShell.override { stdenv = llvmPackages.stdenv; }) {
          nativeBuildInputs =
            with pkgs;
            [
              rustToolchain
              wasmBindgenCli
              dioxus-cli
              pkg-config
              cmake
              # `dx` shells out to wasm-opt for release web builds.
              binaryen
              # `xmllint`, for checking a written file against the UANodeSet XSD
              # independently of our own writer (see CLAUDE.md, "Round-tripping").
              libxml2
              # MCP server (`.mcp.json`) letting agents drive a browser against
              # `dx serve`; wrapped by nixpkgs with nix-patched browsers baked in.
              playwright-mcp
              jetbrains.rust-rover
            ]
            ++ (with llvmPackages; [
              clang-tools
              llvm
              lld
              lldb
            ]);

          buildInputs = with pkgs; [
            openssl
          ];

          NIX_HARDENING_ENABLE = pkgs.lib.concatStringsSep " " wasmHardening;

          shellHook = ''
            mkdir -p ~/.rust-rover/toolchain

            ln -sfn ${rustToolchain}/lib ~/.rust-rover/toolchain
            ln -sfn ${rustToolchain}/bin ~/.rust-rover/toolchain

            export RUST_SRC_PATH="$HOME/.rust-rover/toolchain/lib/rustlib/src/rust/library"
            # Drop into zsh only for interactive `nix develop`. direnv runs this
            # hook in a non-interactive bash while evaluating the environment;
            # spawning a shell there re-triggers the direnv hook and recurses.
            case $- in *i*) zsh ;; esac
          '';
        };
      }
    );
}
