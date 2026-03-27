import type { ComponentProps } from "svelte";
import Root from "./button.svelte";

type ButtonProps = ComponentProps<typeof Root>;
type ButtonVariant = NonNullable<ButtonProps["variant"]>;
type ButtonSize = NonNullable<ButtonProps["size"]>;

export {
	Root,
	type ButtonProps as Props,
	//
	Root as Button,
	type ButtonProps,
	type ButtonSize,
	type ButtonVariant,
};

export declare const buttonVariants: (options?: {
	variant?: ButtonVariant;
	size?: ButtonSize;
}) => string;
