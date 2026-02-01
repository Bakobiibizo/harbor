{
  description = "Harbor - Shard 58 contained P2P chat";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { self, nixpkgs, rust-overlay }:
    let
      system = "x86_64-linux";
      overlays = [ rust-overlay.overlays.default ];
      pkgs = import nixpkgs { inherit system overlays; };
      rust = pkgs.rust-bin.stable.latest.default;
    in {
      packages.${system} = {
        harbor-contained = pkgs.dockerTools.buildImage {
          name = "harbor-shard58";
          tag = "latest";
          
          copyToRoot = pkgs.buildEnv {
            name = "harbor-env";
            paths = [ pkgs.bash pkgs.coreutils ];
          };
          
          config = {
            Cmd = [ "${pkgs.bash}/bin/bash" ];
            WorkingDir = "/app";
            ExposedPorts = { "4001/tcp" = {}; };
          };
        };
        
        default = self.packages.${system}.harbor-contained;
      };
      
      devShells.${system}.default = pkgs.mkShell {
        buildInputs = [
          rust
          pkgs.nodejs_20
          pkgs.cargo
          pkgs.rustc
          pkgs.pkg-config
          pkgs.openssl
          pkgs.sqlite
          pkgs.webkitgtk
          pkgs.gtk3
          pkgs.libsoup
          pkgs.librsvg
        ];
        
        shellHook = ''
          echo "🚢 Harbor - Shard 58 (Contained)"
          echo "FREN: bako-biib (1-800-BAKO-BIIB)"
          echo "Security: Isolated container"
          echo ""
          echo "Commands:"
          echo "  npm install"
          echo "  npm run tauri dev"
        '';
      };
    };
}
