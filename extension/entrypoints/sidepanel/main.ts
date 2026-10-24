import { mount } from "svelte";
import SidePanel from "./SidePanel.svelte";

const target = document.querySelector<HTMLDivElement>("#app");
if (target) mount(SidePanel, { target });
