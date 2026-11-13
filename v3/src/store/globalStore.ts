import { create } from 'zustand';

interface GlobalState {
  legalEntity: string;
  setLegalEntity: (le: string) => void;
  syncStatus: 'online' | 'offline' | 'syncing' | 'error';
  setSyncStatus: (status: 'online' | 'offline' | 'syncing' | 'error') => void;
  networkDriveConnected: boolean;
  setNetworkDriveConnected: (connected: boolean) => void;
}

export const useGlobalStore = create<GlobalState>((set) => ({
  legalEntity: '1618',
  setLegalEntity: (le) => set({ legalEntity: le }),
  syncStatus: 'online',
  setSyncStatus: (status) => set({ syncStatus: status }),
  networkDriveConnected: true,
  setNetworkDriveConnected: (connected) => set({ networkDriveConnected: connected }),
}));
