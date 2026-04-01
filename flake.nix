{
  description = "Harbor — Decentralized P2P chat (Tauri + React + libp2p)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = { url = "github:oxalica/rust-overlay"; inputs.nixpkgs.follows = "nixpkgs"; };
  };

  outputs = { self, nixpkgs, rust-overlay }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" ];
      forAll = f: nixpkgs.lib.genAttrs systems (s: f s);
    in {
      packages = forAll (system: let
        pkgs = import nixpkgs { inherit system; overlays = [ rust-overlay.overlays.default ]; config.allowUnfree = true; };
        rust = pkgs.rust-bin.stable.latest.default;
        npmDeps = pkgs.buildNpmPackage {
          pname = "harbor-frontend";
          version = "1.3.0";
          src = ./.;
          npmDepsHash = "";  # TODO: set after first build
          dontBuild = true;
          installPhase = "cp -r node_modules $out";
        };
      in {
        # Relay server (pure Rust, no JS)
        relay-server = pkgs.rustPlatform.buildRustPackage {
          pname = "harbor-relay-server";
          version = "1.3.0";
          src = ./relay-server;
          cargoLock.lockFile = ./relay-server/Cargo.lock;
          buildInputs = [ pkgs.openssl ];
          nativeBuildInputs = [ pkgs.pkg-config ];
        };

        # Tauri backend (Rust)
        tauri-backend = pkgs.rustPlatform.buildRustPackage {
          pname = "harbor-tauri";
          version = "1.3.0";
          src = ./src-tauri;
          cargoLock.lockFile = ./src-tauri/Cargo.lock;
          buildInputs = with pkgs; [ openssl libsoup_2_4 webkitgtk_4_1 glib-networking ];
          nativeBuildInputs = with pkgs; [ pkg-config ];
        };

        default = self.packages.${system}.relay-server;
      });

      devShells = forAll (system: let
        pkgs = import nixpkgs { inherit system; overlays = [ rust-overlay.overlays.default ]; config.allowUnfree = true; };
      in {
        default = pkgs.mkShell {
          buildInputs = with pkgs; [
            rust-bin.stable.latest.default pkg-config openssl
            libsoup_2_4 webkitgtk_4_1 glib-networking
            nodejs_22 nodePackages.npm
          ];
          shellHook = ''
            echo "Harbor dev shell"
            echo "  npm install && npm run tauri dev"
          '';
        };
      });
    };
}
