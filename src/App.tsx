import "@/styles/app.css";

import { WindowFrame } from "@/components/window/WindowFrame";
import Compressor from "@/features/compression/compressor";
import { useCompressionStoreInit } from "@/features/compression/store/compression-init";

function App() {
  useCompressionStoreInit();

  return (
    <WindowFrame>
      <Compressor />
    </WindowFrame>
  );
}

export default App;
