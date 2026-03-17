export interface ProviderEditorFormState {
  baseUrl: string;
  region: string;
  weight: string;
  disabled: boolean;
}

export interface ProviderRegistryFormState {
  name: string;
  format: 'openai' | 'claude' | 'gemini';
  upstream: string;
  apiKey: string;
  baseUrl: string;
  models: string;
  disabled: boolean;
}

export interface ProviderAtlasProtocolFacts {
  publicRoutes: number;
  providerRoutes: number;
  nativeSurfaces: number;
  adaptedSurfaces: number;
}

export interface ProviderAtlasModelInventoryItem {
  id: string;
  provider: string;
  upstream: string;
  probe: string;
}
