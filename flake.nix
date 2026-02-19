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
                sha256 = "sha256-SBKjxhC6zHTu0SyJwxLlQHItzMzYZ71VCWQC2hOzpRY=";
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

            packages.default = naersk'.buildPackage rec {
              src = ./.;
              nativeBuildInputs = with pkgs; [
                autoPatchelfHook
                libnotify
                glib
                protobuf
              ];
            };

            devShells.default = pkgs.mkShell {
              nativeBuildInputs = with pkgs; [
                cargo
                pkg-config
                toolchain
                libnotify
                glib
                protobuf
              ];
              RUST_SRC_PATH = pkgs.rustPlatform.rustLibSrc;
              LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath [
                pkgs.libnotify
                pkgs.glib
              ];
            };
          };
      }
    );
}
