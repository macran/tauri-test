import { useEffect, useState } from "react";
import { twApi } from "../api";

export function useIsAdmin() {
  const [admin, setAdmin] = useState(false);
  useEffect(() => {
    twApi.isAdmin().then(setAdmin).catch(() => setAdmin(false));
  }, []);
  return admin;
}
