import { createContext, useContext, type ReactNode } from "react";

import type { Client } from "./client";

const ClientContext = createContext<Client | null>(null);

/** Provides the API client to the component tree (tests inject a fake). */
export function ClientProvider({ client, children }: { client: Client; children: ReactNode }) {
  return <ClientContext.Provider value={client}>{children}</ClientContext.Provider>;
}

/** The API client. Throws if used outside a `ClientProvider`. */
export function useClient(): Client {
  const client = useContext(ClientContext);
  if (!client) throw new Error("useClient must be used inside <ClientProvider>");
  return client;
}
