import { Routes, Route, Navigate } from "react-router-dom";
import { Dashboard } from "./pages/Dashboard";
import { Services } from "./pages/Services";
import { Firewall } from "./pages/Firewall";
import { Network } from "./pages/Network";
import { Settings } from "./pages/Settings";
import { Layout } from "./components/Layout";
import { DaemonProvider } from "./lib/daemon-context";

export default function App() {
  return (
    <DaemonProvider>
      <Routes>
        <Route path="/" element={<Layout />}>
          <Route index element={<Dashboard />} />
          <Route path="services" element={<Services />} />
          <Route path="firewall" element={<Firewall />} />
          <Route path="network" element={<Network />} />
          <Route path="settings" element={<Settings />} />
          <Route path="*" element={<Navigate to="/" replace />} />
        </Route>
      </Routes>
    </DaemonProvider>
  );
}
