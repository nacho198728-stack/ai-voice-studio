import { desktopApi } from "./api";
import { mountDesktop } from "./app";
import "./styles.css";

mountDesktop(document.body, desktopApi);
