{
  description = "Type-safe time-value-of-money calculations in Rust";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    git-hooks = {
      url = "github:cachix/git-hooks.nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    okf-tools.url = "github:ojhermann-org/okf-tools/1757bb3a02d3a1e774a7556aa04315c9ce4aeca7";
  };

  outputs =
    {
      nixpkgs,
      rust-overlay,
      git-hooks,
      okf-tools,
      ...
    }:
    let
      systems = [
        "aarch64-darwin"
        "x86_64-linux"
        "aarch64-linux"
      ];

      mkEnv =
        system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ (import rust-overlay) ];
          };

          rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

          # A second component, not a second pin: most of rustfmt.toml is
          # nightly-only and stable ignores it silently.
          nightlyRustfmt = pkgs.rust-bin.nightly."2026-08-01".rustfmt;

          okfGraph = okf-tools.packages.${system}.default;

          fmtCheck = pkgs.writeShellScriptBin "tv-fmt-check" ''
            export RUSTFMT="${nightlyRustfmt}/bin/rustfmt"
            exec ${rustToolchain}/bin/cargo fmt --all -- --check
          '';

          bundleCheck = pkgs.writeShellScriptBin "tv-bundle-check" ''
            exec ${okfGraph}/bin/okf-graph "''${1:-docs/knowledge}"
          '';

          preCommit = git-hooks.lib.${system}.run {
            src = ./.;
            hooks = {
              # Not git-hooks' own `rustfmt` hook: it runs the stable binary.
              cargo-fmt = {
                enable = true;
                name = "cargo fmt (pinned nightly)";
                entry = "${fmtCheck}/bin/tv-fmt-check";
                language = "system";
                files = "\\.rs$";
                pass_filenames = false;
              };
              okf-graph = {
                enable = true;
                name = "okf-graph (Knowledge Bundle)";
                entry = "${bundleCheck}/bin/tv-bundle-check";
                language = "system";
                files = "^docs/knowledge/";
                pass_filenames = false;
              };
              nixfmt.enable = true;
              typos.enable = true;
              trim-trailing-whitespace.enable = true;
              end-of-file-fixer.enable = true;
              check-toml.enable = true;
              check-merge-conflicts.enable = true;
              detect-private-keys.enable = true;
            };
          };
        in
        {
          inherit
            pkgs
            rustToolchain
            nightlyRustfmt
            okfGraph
            fmtCheck
            bundleCheck
            preCommit
            ;
        };

      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f (mkEnv system));
    in
    {
      checks = forAllSystems (env: {
        pre-commit = env.preCommit;
      });

      devShells = forAllSystems (
        env:
        let
          inherit (env) pkgs;
        in
        {
          default = pkgs.mkShell {
            packages = [
              env.rustToolchain
              env.okfGraph
              env.fmtCheck
              env.bundleCheck
              pkgs.bacon
              pkgs.cargo-nextest
              pkgs.cargo-deny
              pkgs.nixfmt
            ];
            buildInputs =
              env.preCommit.enabledPackages ++ nixpkgs.lib.optionals pkgs.stdenv.isDarwin [ pkgs.libiconv ];

            RUSTFMT = "${env.nightlyRustfmt}/bin/rustfmt";

            inherit (env.preCommit) shellHook;
          };
        }
      );

      formatter = forAllSystems (env: env.pkgs.nixfmt);
    };
}
