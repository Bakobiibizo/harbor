import type {
  GameDiscoveryPreview,
  GameDiscoveryResult,
  GameInstallation,
  GamePackagePreview,
  GameRuntimeBundle,
  GameSigningDelivery,
  StoreGamePreview,
} from '../types';
import { invokeCommand } from './command';

export function approveGameSigningRequest(approvalId: string): Promise<GameSigningDelivery> {
  return invokeCommand('approve_game_signing_request', { approvalId });
}

export function inspectGamePackage(filePath: string): Promise<GamePackagePreview> {
  return invokeCommand('inspect_game_package', { filePath });
}

export function importGamePackage(
  filePath: string,
  approvedPermissions: string[],
): Promise<GameInstallation> {
  return invokeCommand('import_game_package', { approvedPermissions, filePath });
}

export function configureGameDiscoveryFolder(folderPath: string): Promise<void> {
  return invokeCommand('configure_game_discovery_folder', { folderPath });
}

export function getGameDiscoveryFolder(): Promise<string | null> {
  return invokeCommand('get_game_discovery_folder');
}

export function inspectDiscoveredGamePackages(): Promise<GameDiscoveryPreview[]> {
  return invokeCommand('inspect_discovered_game_packages');
}

export function discoverGamePackages(
  approvedPermissions: string[],
): Promise<GameDiscoveryResult[]> {
  return invokeCommand('discover_game_packages', { approvedPermissions });
}

export function getStoreGameMetadata(gameId: string, versionId: string): Promise<StoreGamePreview> {
  return invokeCommand('get_store_game_metadata', { gameId, versionId });
}

export function installStoreGame(
  gameId: string,
  versionId: string,
  approvedPermissions: string[],
): Promise<GameInstallation> {
  return invokeCommand('install_store_game', { approvedPermissions, gameId, versionId });
}

export function loadGameRuntime(gameId: string): Promise<GameRuntimeBundle> {
  return invokeCommand('load_game_runtime', { gameId });
}

export function recordGameLaunch(gameId: string): Promise<void> {
  return invokeCommand('record_game_launch', { gameId });
}

export function listInstalledGames(): Promise<GameInstallation[]> {
  return invokeCommand('list_installed_games');
}

export function uninstallGame(gameId: string): Promise<void> {
  return invokeCommand('uninstall_game', { gameId });
}

export function readGameSave(gameId: string, slot: string): Promise<number[] | null> {
  return invokeCommand('read_game_save', { gameId, slot });
}

export function writeGameSave(gameId: string, slot: string, data: number[]): Promise<void> {
  return invokeCommand('write_game_save', { data, gameId, slot });
}

export function deleteGameSaves(gameId: string): Promise<number> {
  return invokeCommand('delete_game_saves', { gameId });
}
