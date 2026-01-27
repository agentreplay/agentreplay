// Copyright 2025 Sushanth (https://github.com/sushanthpy)
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

import { Store } from '@tauri-apps/plugin-store';

const STORE_PATH = 'flowtrace.ui.store';
const STORE_KEY = 'selectedProjectId';
const LOCAL_STORAGE_KEY = 'flowtrace:selectedProject';

const isDesktopRuntime = () =>
  typeof window !== 'undefined' && Boolean((window as any).__TAURI_IPC__);

let storePromise: Promise<Store | null> | null = null;

async function getStoreInstance(): Promise<Store | null> {
  if (!isDesktopRuntime()) {
    return null;
  }

  if (!storePromise) {
    storePromise = Store.load(STORE_PATH).catch((error) => {
      console.warn('Failed to load Tauri store', error);
      return null;
    });
  }

  return storePromise;
}

export async function getStoredProjectId(): Promise<string | null> {
  let value: string | undefined;

  try {
    const store = await getStoreInstance();
    value = store ? await store.get<string>(STORE_KEY) : undefined;
  } catch (error) {
    console.warn('Failed to read project from store', error);
  }

  if (value) {
    return value;
  }

  if (typeof window !== 'undefined') {
    return window.localStorage.getItem(LOCAL_STORAGE_KEY);
  }

  return null;
}

export async function persistProjectId(projectId: string) {
  try {
    const store = await getStoreInstance();
    if (store) {
      await store.set(STORE_KEY, projectId);
      await store.save();
    }
  } catch (error) {
    console.warn('Failed to persist project', error);
  }

  if (typeof window !== 'undefined') {
    window.localStorage.setItem(LOCAL_STORAGE_KEY, projectId);
  }
}

export async function clearStoredProject() {
  try {
    const store = await getStoreInstance();
    if (store) {
      await store.delete(STORE_KEY);
      await store.save();
    }
  } catch (error) {
    console.warn('Failed to clear project state', error);
  }

  if (typeof window !== 'undefined') {
    window.localStorage.removeItem(LOCAL_STORAGE_KEY);
  }
}
