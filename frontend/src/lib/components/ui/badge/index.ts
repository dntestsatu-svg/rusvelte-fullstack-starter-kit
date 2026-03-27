import type { ComponentProps } from "svelte";
import Root from "./badge.svelte";

type BadgeProps = ComponentProps<typeof Root>;

export { Root, Root as Badge };
export type BadgeVariant = NonNullable<BadgeProps["variant"]>;
export declare const badgeVariants: (options?: { variant?: BadgeVariant }) => string;
