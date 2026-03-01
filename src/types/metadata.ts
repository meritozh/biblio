export type MetadataType = 'text' | 'number' | 'date' | 'boolean';

export interface Metadata {
  id: number;
  fileId: number;
  key: string;
  value: string;
  dataType: MetadataType;
}