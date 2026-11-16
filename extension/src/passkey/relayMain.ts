// Isolated-world entry point at document start (see manifest).
import type { ContentMessage, Reply } from "../shared/messages";
import { startRelay } from "./relay";

startRelay(window, (message: ContentMessage) => chrome.runtime.sendMessage<ContentMessage, Reply>(message));
