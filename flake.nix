{
  inputs = {
    flake-utils.url = "github:numtide/flake-utils";

    naersk.url = "github:nix-community/naersk";

    nixgl.url = "github:nix-community/nixGL";
    nixgl.inputs.flake-utils.follows = "flake-utils";
    nixgl.inputs.nixpkgs.follows = "nixpkgs";

    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.05";

    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs =
    {
      self,
      flake-utils,
      naersk,
      nixgl,
      nixpkgs,
      rust-overlay,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = (nixpkgs.legacyPackages.${system}.extend (import rust-overlay)).extend nixgl.overlay;

        rust-toolchain = pkgs.pkgsBuildHost.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

        naersk' = pkgs.callPackage naersk {
          cargo = rust-toolchain;
          rustc = rust-toolchain;
        };

        libraries = with pkgs; [
          libGL
          xorg.libX11
          xorg.libXi
          libxkbcommon
        ];
        physarum-36p-rs-unwrapped = naersk'.buildPackage {
          src = ./.;
          nativeBuildInputs = libraries;
          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath libraries;
        };

        physarum = pkgs.writeShellApplication {
          name = "physarum";
          runtimeInputs = [
            pkgs.nixgl.auto.nixGLDefault
            physarum-36p-rs-unwrapped
          ];
          text = ''
            nixGL physarum "$@"
          '';
        };
      in
      {
        packages = {
          inherit physarum-36p-rs-unwrapped physarum;
          default = physarum;
        };

        devShells.default = pkgs.mkShell {
          buildInputs = [ rust-toolchain ];
          nativeBuildInputs =
            (with pkgs; [
              # needed for running
              pkgs.nixgl.auto.nixGLNvidia
              # nice to have
              just
              # For debugging
              vscode-extensions.vadimcn.vscode-lldb.adapter
            ])
            ++ libraries;
          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath libraries;
          shellHook = ''
            # Needed for nixGL to work correctly
            export __GLX_VENDOR_LIBRARY_NAME=nvidia
          '';
        };
      }
    );
}
