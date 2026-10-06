import { createContext, use, useEffect, useState, type ReactNode } from "react";
import { api } from "@/lib/api";

const PrivilegeContext = createContext<{ elevated: boolean }>({ elevated: false });

export function PrivilegeProvider({ children }: { children: ReactNode }) {
  const [elevated, setElevated] = useState(false);
  useEffect(() => {
    api.getPrivilege().then((p) => setElevated(p.elevated)).catch(() => setElevated(false));
  }, []);
  return <PrivilegeContext value={{ elevated }}>{children}</PrivilegeContext>;
}

export const usePrivilege = () => use(PrivilegeContext);
