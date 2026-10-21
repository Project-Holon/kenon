// Typed wrappers for the commands in app/src/lib.rs.
import { convertFileSrc, invoke } from "@tauri-apps/api/core";

export interface Identity {
  title: string;
  gameCode: string;
  game: "black" | "white" | "other";
  size: number;
}

export type GameCheck =
  | { state: "rom"; identity: Identity; supported: boolean }
  | { state: "folder"; hasArm9: boolean }
  | { state: "problem"; message: string };

export interface GameStatus {
  path: string | null;
  check: GameCheck | null;
  suggestion: string | null;
}

export function gameStatus(): Promise<GameStatus> {
  return invoke("game_status");
}

export function setGame(path: string | null): Promise<GameStatus> {
  return invoke("set_game", { path });
}

export function isReady(status: GameStatus | null): boolean {
  return status?.check?.state === "rom" || status?.check?.state === "folder";
}

/// The URL of an asset bridge path (see core/src/bridge.rs).
export function asset(path: string): string {
  return convertFileSrc(path, "kenon");
}
