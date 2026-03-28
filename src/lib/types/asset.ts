export interface Asset {
  id: number;
  path: string;
  filename: string;
  extension: string;
  folder: string;
  durationMs: number | null;
  sampleRate: number | null;
  channels: number | null;
  bitDepth: number | null;
  fileSize: number;
  category: string | null;
  publisher: string | null;
  favorite: boolean;
  rating: number;
  notes: string | null;
  playCount: number;
}

export interface NewAsset {
  path: string;
  filename: string;
  extension: string;
  folder: string;
  durationMs: number | null;
  sampleRate: number | null;
  channels: number | null;
  bitDepth: number | null;
  fileSize: number;
  category: string | null;
  publisher: string | null;
  modifiedAt: number;
  indexedAt: number;
  peaks: Uint8Array | null;
}

export interface FolderNode {
  name: string;
  path: string;
  count: number;
  children: FolderNode[];
}

export interface Tag {
  id: number;
  name: string;
}

export interface TagWithCount extends Tag {
  count: number;
}
