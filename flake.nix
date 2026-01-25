{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs";
    flake-parts.url = "github:hercules-ci/flake-parts";
    naersk.url = "github:nix-community/naersk";
    # Just used for rustChannelOf
    nixpkgs-mozilla.url = "github:mozilla/nixpkgs-mozilla";
  };

  outputs =
    inputs@{
      self,
      nixpkgs,
      flake-parts,
      naersk,
      nixpkgs-mozilla,
    }:
    flake-parts.lib.mkFlake { inherit inputs; } (
      { ... }:
      {
        systems = [ "x86_64-linux" ];

        perSystem =
          {
            system,
            config,
            pkgs,
            ...
          }:
          let
            toolchain =
              (pkgs.rustChannelOf {
                rustToolchain = ./rust-toolchain.toml;
                sha256 = "sha256-vra6TkHITpwRyA5oBKAHSX0Mi6CBDNQD+ryPSpxFsfg=";
              }).rust;

            naersk' = pkgs.callPackage naersk {
              cargo = toolchain;
              rustc = toolchain;
            };
          in
          {
            _module.args.pkgs = import nixpkgs {
              inherit system;
              overlays = [
                (import nixpkgs-mozilla)
              ];
              config = { };
            };

            packages.default = naersk'.buildPackage {
              src = ./.;
            };

            devShells.default = pkgs.mkShell {
              nativeBuildInputs = with pkgs; [
                cargo
                pkg-config
                toolchain
              ];
              RUST_SRC_PATH = pkgs.rustPlatform.rustLibSrc;
            };
          };
      }
    );
}