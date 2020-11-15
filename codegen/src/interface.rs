use std::iter::FromIterator;

use proc_macro::TokenStream;
use quote::{__private::Span, quote};
use syn::{
    parse_macro_input,
    punctuated::Punctuated,
    token::{Brace, Paren},
    Abi, AngleBracketedGenericArguments, BareFnArg, Binding, Block, Expr, ExprCall, ExprCast,
    ExprField, ExprParen, ExprPath, ExprReference, ExprStruct, ExprUnary, ExprUnsafe, Field,
    FieldValue, FnArg, GenericArgument, GenericParam, Generics, Ident, ImplItem, ImplItemMethod,
    Item, ItemFn, ItemImpl, ItemTrait, LitStr, Member, Pat, PatPath, PatType, Path, PathArguments,
    PathSegment, Receiver, ReturnType, Signature, Stmt, Token, TraitBound, TraitBoundModifier,
    TraitItem, TraitItemMethod, Type, TypeBareFn, TypeParam, TypeParamBound, TypePath, TypePtr,
    TypeTraitObject, UnOp, VisPublic, VisRestricted, Visibility,
};

fn ident(name: &str) -> Ident {
    Ident::new(name, Span::call_site())
}

fn punctuated<A, B>(iter: impl IntoIterator<Item = A>) -> Punctuated<A, B>
where
    Punctuated<A, B>: FromIterator<A>,
{
    iter.into_iter().collect()
}

fn generics_argument(
    args: impl IntoIterator<Item = GenericArgument>,
) -> AngleBracketedGenericArguments {
    AngleBracketedGenericArguments {
        lt_token: Token![<](Span::call_site()),
        args: punctuated(args),
        colon2_token: None,
        gt_token: Token![>](Span::call_site()),
    }
}

fn segment(ident: Ident, arguments: Option<AngleBracketedGenericArguments>) -> PathSegment {
    PathSegment {
        ident,
        arguments: if let Some(arguments) = arguments {
            PathArguments::AngleBracketed(arguments)
        } else {
            PathArguments::None
        },
    }
}

fn path(segments: impl IntoIterator<Item = PathSegment>) -> Path {
    Path {
        leading_colon: None,
        segments: punctuated(segments),
    }
}

fn path_type(segments: impl IntoIterator<Item = PathSegment>) -> TypePath {
    TypePath {
        qself: None,
        path: path(segments),
    }
}

fn pointer_type(mutability: Option<Token![mut]>, ty: Type) -> TypePtr {
    TypePtr {
        star_token: Token![*]([Span::call_site(); 1]),
        const_token: if mutability.is_none() {
            Some(Token![const](Span::call_site()))
        } else {
            None
        },
        mutability,
        elem: Box::new(ty),
    }
}

fn map_type(input: &Type) -> Type {
    match input {
        Type::Reference(reference) => match &*reference.elem {
            Type::Path(path) => {
                if let Some(seg) = path.path.segments.last() {
                    match &seg.ident.to_string() as &str {
                        "CStr" => {
                            return Type::Ptr(pointer_type(
                                reference.mutability.clone(),
                                Type::Path(path_type(vec![
                                    segment(ident("std"), None),
                                    segment(ident("os"), None),
                                    segment(ident("raw"), None),
                                    segment(ident("c_char"), None),
                                ])),
                            ));
                        }
                        _ => {}
                    }
                }
            }

            Type::TraitObject(_) => {
                return Type::Ptr(pointer_type(
                    reference.mutability.clone(),
                    Type::Path(path_type(vec![
                        segment(ident("std"), None),
                        segment(ident("ffi"), None),
                        segment(ident("c_void"), None),
                    ])),
                ))
            }

            _ => {}
        },

        Type::Path(path) => {
            if let Some(seg) = path.path.segments.last() {
                match &seg.ident.to_string() as &str {
                    "Box" => {
                        let args = match &seg.arguments {
                            PathArguments::AngleBracketed(args) => args,
                            other => panic!("{:?}", other),
                        };

                        let arg = match &args.args[0] {
                            GenericArgument::Type(arg) => arg,
                            other => panic!("{:?}", other),
                        };

                        match arg {
                            Type::TraitObject(_) => {
                                return Type::Ptr(pointer_type(
                                    Some(Token![mut](Span::call_site())),
                                    Type::Path(path_type(vec![
                                        segment(ident("std"), None),
                                        segment(ident("ffi"), None),
                                        segment(ident("c_void"), None),
                                    ])),
                                ));
                            }

                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        }

        _ => {}
    }

    input.clone()
}

fn map_input(input: Expr, ty: &Type) -> Expr {
    match ty {
        Type::Reference(reference) => match &*reference.elem {
            Type::Path(pat) => {
                if let Some(seg) = pat.path.segments.last() {
                    if seg.ident.to_string() == "CStr" {
                        return Expr::Call(ExprCall {
                            attrs: Vec::new(),
                            func: Box::new(Expr::Field(ExprField {
                                attrs: Vec::new(),
                                base: Box::new(input),
                                dot_token: Token![.](Span::call_site()),
                                member: Member::Named(ident("as_ptr")),
                            })),
                            paren_token: Paren(Span::call_site()),
                            args: Punctuated::new(),
                        });
                    }
                }
            }

            Type::TraitObject(obj) => {
                let bound = match &obj.bounds[0] {
                    TypeParamBound::Trait(bound) => bound,
                    other => panic!("{:?}", other),
                };

                let name = match bound.path.segments.last() {
                    Some(segment) => &segment.ident,
                    None => panic!(),
                };

                let vtable_name = Ident::new(&format!("I{}", name), Span::call_site());
                let class_name = Ident::new(&format!("C{}", name), Span::call_site());
                let ty = ty.clone();

                // TODO: Move the static vtable somewhere it can be shared
                // between all invocations so they do not bloat the binary
                return Expr::Verbatim(quote! {{
                    static VTABLE: #vtable_name = #name::vtable::<#ty, _>();

                    let instance = Box::new(#class_name {
                        vtable: &VTABLE as *const #vtable_name,
                        instance: #input
                    });

                    let ptr = Box::into_raw(instance);
                    log::trace!(concat!("into_raw ", stringify!(#class_name), " {:?}"), ptr);
                    ptr as *mut std::ffi::c_void
                }});
            }

            _ => {}
        },

        Type::Path(pat) => {
            if let Some(seg) = pat.path.segments.last() {
                match &seg.ident.to_string() as &str {
                    "Box" => {
                        let args = match &seg.arguments {
                            PathArguments::AngleBracketed(args) => args,
                            other => panic!("{:?}", other),
                        };

                        if let GenericArgument::Type(Type::TraitObject(obj)) = &args.args[0] {
                            let bound = match &obj.bounds[0] {
                                TypeParamBound::Trait(bound) => bound,
                                other => panic!("{:?}", other),
                            };

                            let name = match bound.path.segments.last() {
                                Some(segment) => &segment.ident,
                                None => panic!(),
                            };

                            let vtable_name = Ident::new(&format!("I{}", name), Span::call_site());
                            let class_name = Ident::new(&format!("C{}", name), Span::call_site());
                            let ty = ty.clone();

                            return Expr::Verbatim(quote! {{
                                static VTABLE: #vtable_name = #name::vtable::<#ty, _>();
                                let instance = Box::new(#class_name {
                                    vtable: &VTABLE as *const #vtable_name,
                                    instance: #input
                                });

                                let ptr = Box::into_raw(instance);
                                log::trace!(concat!("into_raw ", stringify!(#class_name), " {:?}"), ptr);
                                ptr as *mut std::ffi::c_void
                            }});
                        }
                    }
                    _ => {}
                }
            }
        }

        _ => {}
    }

    input
}

fn map_output(input: Expr, ty: &Type) -> Expr {
    match ty {
        Type::Reference(reference) => match &*reference.elem {
            Type::Path(pat) => {
                if let Some(seg) = pat.path.segments.last() {
                    match &seg.ident.to_string() as &str {
                        "CStr" => {
                            return Expr::Call(ExprCall {
                                attrs: Vec::new(),
                                func: Box::new(Expr::Path(ExprPath {
                                    attrs: Vec::new(),
                                    qself: None,
                                    path: path(vec![
                                        segment(ident("std"), None),
                                        segment(ident("ffi"), None),
                                        segment(ident("CStr"), None),
                                        segment(ident("from_ptr"), None),
                                    ]),
                                })),
                                paren_token: Paren(Span::call_site()),
                                args: punctuated(vec![input]),
                            });
                        }
                        _ => {}
                    }
                }
            }

            Type::TraitObject(obj) => {
                return Expr::Verbatim(quote! {
                    &mut crate::foreign::Foreign::<#obj>::with(#input)
                })
            }

            _ => {}
        },

        Type::Path(pat) => {
            if let Some(seg) = pat.path.segments.last() {
                match &seg.ident.to_string() as &str {
                    "Box" => {
                        let args = match &seg.arguments {
                            PathArguments::AngleBracketed(args) => args,
                            other => panic!("{:?}", other),
                        };

                        let arg = match &args.args[0] {
                            GenericArgument::Type(arg) => arg,
                            other => panic!("{:?}", other),
                        };

                        match arg {
                            Type::TraitObject(obj) => {
                                return Expr::Verbatim(quote! {
                                    Box::new(crate::foreign::Foreign::<#obj>::with(#input))
                                });
                            }

                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        }

        _ => {}
    }

    input
}

fn map_self_output(input: &Receiver, class_name: &Ident) -> Expr {
    if input.reference.is_some() {
        Expr::Reference(ExprReference {
            attrs: Vec::new(),
            and_token: Token![&](Span::call_site()),
            raw: Default::default(),
            mutability: if input.mutability.is_some() {
                Some(Token![mut](Span::call_site()))
            } else {
                None
            },
            expr: Box::new(Expr::Field(ExprField {
                attrs: Vec::new(),
                base: Box::new(Expr::Paren(ExprParen {
                    attrs: Vec::new(),
                    paren_token: Paren(Span::call_site()),
                    expr: Box::new(Expr::Unary(ExprUnary {
                        attrs: Vec::new(),
                        op: UnOp::Deref(Token![*](Span::call_site())),
                        expr: Box::new(Expr::Paren(ExprParen {
                            attrs: Vec::new(),
                            paren_token: Paren(Span::call_site()),
                            expr: Box::new(Expr::Cast(ExprCast {
                                attrs: Vec::new(),
                                expr: Box::new(Expr::Path(ExprPath {
                                    attrs: Vec::new(),
                                    qself: None,
                                    path: path(vec![segment(ident("this"), None)]),
                                })),
                                as_token: Token![as](Span::call_site()),
                                ty: Box::new(Type::Ptr(pointer_type(
                                    input.mutability.clone(),
                                    Type::Path(path_type(vec![segment(
                                        class_name.clone(),
                                        Some(generics_argument(vec![GenericArgument::Type(
                                            Type::Path(TypePath {
                                                qself: None,
                                                path: path(vec![segment(ident("P"), None)]),
                                            }),
                                        )])),
                                    )])),
                                ))),
                            })),
                        })),
                    })),
                })),
                dot_token: Token![.](Span::call_site()),
                member: Member::Named(ident("instance")),
            })),
        })
    } else {
        Expr::Verbatim(quote! {
            *Box::from_raw(this as *mut #class_name<P>).instance
        })
    }
}

fn impl_trait(
    trait_name: Ident,
    self_path: TypePath,
    generics: &[Ident],
    items: Vec<ImplItem>,
) -> ItemImpl {
    ItemImpl {
        attrs: Vec::new(),
        defaultness: None,
        unsafety: None,
        impl_token: Token![impl](Span::call_site()),
        generics: if generics.is_empty() {
            Generics {
                lt_token: None,
                params: Punctuated::new(),
                gt_token: None,
                where_clause: None,
            }
        } else {
            Generics {
                lt_token: Some(Token![<](Span::call_site())),
                params: generics
                    .iter()
                    .map(|ident| {
                        GenericParam::Type(TypeParam {
                            attrs: Vec::new(),
                            ident: ident.clone(),
                            colon_token: None,
                            bounds: Punctuated::new(),
                            eq_token: None,
                            default: None,
                        })
                    })
                    .collect(),
                gt_token: Some(Token![>](Span::call_site())),
                where_clause: None,
            }
        },
        trait_: Some((
            None,
            path(vec![segment(trait_name, None)]),
            Token![for](Span::call_site()),
        )),
        self_ty: Box::new(Type::Path(self_path)),
        brace_token: Brace(Span::call_site()),
        items,
    }
}

fn is_owned(method: &TraitItemMethod) -> bool {
    method.sig.inputs.iter().any(|arg| {
        if let FnArg::Receiver(recv) = arg {
            recv.reference.is_none()
        } else {
            false
        }
    })
}

fn is_mutable(method: &TraitItemMethod) -> bool {
    method.sig.inputs.iter().any(|arg| {
        if let FnArg::Receiver(recv) = arg {
            recv.mutability.is_some()
        } else {
            false
        }
    })
}

fn vtable_shim(method: &TraitItemMethod, trait_name: &Ident, class_name: &Ident) -> ItemFn {
    let mut container_bounds = vec![TypeParamBound::Trait(TraitBound {
        paren_token: None,
        modifier: TraitBoundModifier::None,
        lifetimes: None,
        path: path(vec![
            segment(ident("std"), None),
            segment(ident("ops"), None),
            segment(
                ident("Deref"),
                Some(generics_argument(vec![GenericArgument::Binding(Binding {
                    ident: ident("Target"),
                    eq_token: Token![=](Span::call_site()),
                    ty: Type::Path(TypePath {
                        qself: None,
                        path: path(vec![segment(ident("T"), None)]),
                    }),
                })])),
            ),
        ]),
    })];

    if is_mutable(method) {
        container_bounds.push(TypeParamBound::Trait(TraitBound {
            paren_token: None,
            modifier: TraitBoundModifier::None,
            lifetimes: None,
            path: path(vec![
                segment(ident("std"), None),
                segment(ident("ops"), None),
                segment(ident("DerefMut"), None),
            ]),
        }));
    }

    let mut content_bounds = vec![TypeParamBound::Trait(TraitBound {
        paren_token: None,
        modifier: TraitBoundModifier::None,
        lifetimes: None,
        path: path(vec![segment(trait_name.clone(), None)]),
    })];

    if !is_owned(method) {
        content_bounds.push(TypeParamBound::Trait(TraitBound {
            paren_token: None,
            modifier: TraitBoundModifier::Maybe(Token![?](Span::call_site())),
            lifetimes: None,
            path: path(vec![segment(ident("Sized"), None)]),
        }));
    }

    ItemFn {
        attrs: Vec::new(),
        vis: Visibility::Inherited,
        sig: Signature {
            constness: method.sig.constness.clone(),
            asyncness: method.sig.asyncness.clone(),
            unsafety: method.sig.unsafety.clone(),
            abi: Some(Abi {
                extern_token: Token![extern](Span::call_site()),
                name: Some(LitStr::new("thiscall", Span::call_site())),
            }),
            fn_token: method.sig.fn_token.clone(),
            ident: method.sig.ident.clone(),
            generics: Generics {
                lt_token: Some(Token![<](Span::call_site())),
                params: punctuated(vec![
                    GenericParam::Type(TypeParam {
                        attrs: Vec::new(),
                        ident: ident("P"),
                        colon_token: Some(Token![:](Span::call_site())),
                        bounds: punctuated(container_bounds),
                        eq_token: None,
                        default: None,
                    }),
                    GenericParam::Type(TypeParam {
                        attrs: Vec::new(),
                        ident: ident("T"),
                        colon_token: Some(Token![:](Span::call_site())),
                        bounds: punctuated(content_bounds),
                        eq_token: None,
                        default: None,
                    }),
                ]),
                where_clause: None,
                gt_token: Some(Token![>](Span::call_site())),
            },
            paren_token: method.sig.paren_token.clone(),
            inputs: method
                .sig
                .inputs
                .iter()
                .map(|input| match input {
                    FnArg::Receiver(receiver) => FnArg::Typed(PatType {
                        attrs: Vec::new(),
                        pat: Box::new(Pat::Path(PatPath {
                            attrs: Vec::new(),
                            qself: None,
                            path: path(vec![segment(ident("this"), None)]),
                        })),
                        colon_token: Token![:](Span::call_site()),
                        ty: Box::new(Type::Ptr(pointer_type(
                            receiver.mutability.clone(),
                            Type::Path(path_type(vec![
                                segment(ident("std"), None),
                                segment(ident("ffi"), None),
                                segment(ident("c_void"), None),
                            ])),
                        ))),
                    }),
                    FnArg::Typed(input) => FnArg::Typed(PatType {
                        attrs: Vec::new(),
                        pat: input.pat.clone(),
                        colon_token: Token![:](Span::call_site()),
                        ty: Box::new(map_type(&*input.ty)),
                    }),
                })
                .collect(),
            variadic: method.sig.variadic.clone(),
            output: match &method.sig.output {
                ReturnType::Default => ReturnType::Default,
                ReturnType::Type(token, ty) => {
                    ReturnType::Type(token.clone(), Box::new(map_type(ty)))
                }
            },
        },
        block: Box::new(Block {
            brace_token: Brace(Span::call_site()),
            stmts: vec![Stmt::Expr(Expr::Unsafe(ExprUnsafe {
                attrs: Vec::new(),
                unsafe_token: Token![unsafe](Span::call_site()),
                block: Block {
                    brace_token: Brace(Span::call_site()),
                    stmts: vec![
                        Stmt::Expr(Expr::Verbatim({
                            let name = method.sig.ident.clone();
                            quote! {
                                log::trace!(concat!(stringify!(#trait_name), "::", stringify!(#name)));
                            }
                        })),
                        Stmt::Expr({
                            let expr = Expr::Call(ExprCall {
                                attrs: Vec::new(),
                                func: Box::new(Expr::Path(ExprPath {
                                    attrs: Vec::new(),
                                    qself: None,
                                    path: path(vec![
                                        segment(ident("T"), None),
                                        segment(method.sig.ident.clone(), None),
                                    ]),
                                })),
                                paren_token: Paren(method.sig.ident.span()),
                                args: method
                                    .sig
                                    .inputs
                                    .iter()
                                    .map(|input| match input {
                                        FnArg::Receiver(input) => {
                                            map_self_output(input, &class_name)
                                        }
                                        FnArg::Typed(input) => match &*input.pat {
                                            Pat::Ident(id) => map_output(
                                                Expr::Path(ExprPath {
                                                    attrs: Vec::new(),
                                                    qself: None,
                                                    path: path(vec![segment(
                                                        id.ident.clone(),
                                                        None,
                                                    )]),
                                                }),
                                                &input.ty,
                                            ),
                                            pat => panic!("{:?}", pat),
                                        },
                                    })
                                    .collect(),
                            });

                            match &method.sig.output {
                                ReturnType::Default => expr,
                                ReturnType::Type(_, ty) => map_input(expr, ty),
                            }
                        }),
                    ],
                },
            }))],
        }),
    }
}

fn vtable_impl(input: &ItemTrait) -> ItemImpl {
    let name = input.ident.clone();
    let vtable_name = Ident::new(&format!("I{}", name), Span::call_site());
    let class_name = Ident::new(&format!("C{}", name), Span::call_site());

    let vtable_shims: Vec<_> = input
        .items
        .iter()
        .map(|item| match item {
            TraitItem::Method(method) => vtable_shim(method, &name, &class_name),
            item => panic!("{:?}", item),
        })
        .collect();

    let vtable_entries: Punctuated<_, Token![,]> = input
        .items
        .iter()
        .map(|item| match item {
            TraitItem::Method(method) => {
                let ident = method.sig.ident.clone();
                FieldValue {
                    attrs: Vec::new(),
                    member: Member::Named(method.sig.ident.clone()),
                    colon_token: Some(Token![:](Span::call_site())),
                    expr: Expr::Verbatim(quote! {
                        #ident::<P, T>
                    }),
                }
            }
            other => panic!("{:?}", other),
        })
        .collect();

    let mut container_bounds = vec![TypeParamBound::Trait(TraitBound {
        paren_token: None,
        modifier: TraitBoundModifier::None,
        lifetimes: None,
        path: path(vec![
            segment(ident("std"), None),
            segment(ident("ops"), None),
            segment(
                ident("Deref"),
                Some(generics_argument(vec![GenericArgument::Binding(Binding {
                    ident: ident("Target"),
                    eq_token: Token![=](Span::call_site()),
                    ty: Type::Path(TypePath {
                        qself: None,
                        path: path(vec![segment(ident("T"), None)]),
                    }),
                })])),
            ),
        ]),
    })];

    let has_mutable = input.items.iter().any(|item| {
        if let TraitItem::Method(method) = item {
            is_mutable(method)
        } else {
            false
        }
    });

    if has_mutable {
        container_bounds.push(TypeParamBound::Trait(TraitBound {
            paren_token: None,
            modifier: TraitBoundModifier::None,
            lifetimes: None,
            path: path(vec![
                segment(ident("std"), None),
                segment(ident("ops"), None),
                segment(ident("DerefMut"), None),
            ]),
        }));
    }

    let mut content_bounds = vec![TypeParamBound::Trait(TraitBound {
        paren_token: None,
        modifier: TraitBoundModifier::None,
        lifetimes: None,
        path: path(vec![segment(name.clone(), None)]),
    })];

    let has_owned = input.items.iter().any(|item| {
        if let TraitItem::Method(method) = item {
            is_owned(method)
        } else {
            false
        }
    });

    if !has_owned {
        content_bounds.push(TypeParamBound::Trait(TraitBound {
            paren_token: None,
            modifier: TraitBoundModifier::Maybe(Token![?](Span::call_site())),
            lifetimes: None,
            path: path(vec![segment(ident("Sized"), None)]),
        }));
    }

    ItemImpl {
        attrs: Vec::new(),
        defaultness: None,
        unsafety: None,
        impl_token: Token![impl](Span::call_site()),
        generics: Generics {
            lt_token: None,
            params: Punctuated::new(),
            gt_token: None,
            where_clause: None,
        },
        trait_: None,
        self_ty: Box::new(Type::TraitObject(TypeTraitObject {
            dyn_token: Some(Token![dyn](Span::call_site())),
            bounds: punctuated(vec![TypeParamBound::Trait(TraitBound {
                paren_token: None,
                modifier: TraitBoundModifier::None,
                lifetimes: None,
                path: path(vec![segment(name.clone(), None)]),
            })]),
        })),
        brace_token: Brace(Span::call_site()),
        items: vec![ImplItem::Method(ImplItemMethod {
            attrs: Vec::new(),
            vis: Visibility::Restricted(VisRestricted {
                pub_token: Token![pub](Span::call_site()),
                paren_token: Paren(Span::call_site()),
                in_token: None,
                path: Box::new(path(vec![segment(ident("crate"), None)])),
            }),
            defaultness: None,
            sig: Signature {
                constness: Some(Token![const](Span::call_site())),
                asyncness: None,
                unsafety: None,
                abi: None,
                fn_token: Token![fn](Span::call_site()),
                ident: ident("vtable"),
                generics: Generics {
                    lt_token: Some(Token![<](Span::call_site())),
                    params: punctuated(vec![
                        GenericParam::Type(TypeParam {
                            attrs: Vec::new(),
                            ident: ident("P"),
                            colon_token: Some(Token![:](Span::call_site())),
                            bounds: punctuated(container_bounds),
                            eq_token: None,
                            default: None,
                        }),
                        GenericParam::Type(TypeParam {
                            attrs: Vec::new(),
                            ident: ident("T"),
                            colon_token: Some(Token![:](Span::call_site())),
                            bounds: punctuated(content_bounds),
                            eq_token: None,
                            default: None,
                        }),
                    ]),
                    gt_token: Some(Token![>](Span::call_site())),
                    where_clause: None,
                },
                paren_token: Paren(Span::call_site()),
                inputs: punctuated(None),
                variadic: None,
                output: ReturnType::Type(
                    Token![->](Span::call_site()),
                    Box::new(Type::Path(path_type(vec![segment(
                        vtable_name.clone(),
                        None,
                    )]))),
                ),
            },
            block: Block {
                brace_token: Brace(Span::call_site()),
                stmts: vtable_shims
                    .into_iter()
                    .map(|item| Stmt::Item(Item::Fn(item)))
                    .chain(Some(Stmt::Expr(Expr::Struct(ExprStruct {
                        attrs: Vec::new(),
                        path: path(vec![segment(vtable_name.clone(), None)]),
                        brace_token: Brace(Span::call_site()),
                        fields: vtable_entries,
                        dot2_token: None,
                        rest: None,
                    }))))
                    .collect(),
            },
        })],
    }
}

pub fn interface(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ItemTrait);

    let name = input.ident.clone();
    let vtable_name = Ident::new(&format!("I{}", name), Span::call_site());
    let class_name = Ident::new(&format!("C{}", name), Span::call_site());

    let vtable_fields: Punctuated<_, Token![,]> = input
        .items
        .iter()
        .map(|item| match item {
            TraitItem::Method(method) => Field {
                attrs: Vec::new(),
                vis: Visibility::Public(VisPublic {
                    pub_token: Token![pub](Span::call_site()),
                }),
                ident: Some(method.sig.ident.clone()),
                colon_token: None,
                ty: Type::BareFn(TypeBareFn {
                    lifetimes: None,
                    unsafety: None,
                    abi: Some(Abi {
                        extern_token: Token![extern](Span::call_site()),
                        name: Some(LitStr::new("thiscall", Span::call_site())),
                    }),
                    fn_token: method.sig.fn_token.clone(),
                    paren_token: method.sig.paren_token.clone(),
                    inputs: method
                        .sig
                        .inputs
                        .iter()
                        .map(|input| match input {
                            FnArg::Receiver(receiver) => BareFnArg {
                                attrs: Vec::new(),
                                name: None,
                                ty: Type::Ptr(pointer_type(
                                    receiver.mutability.clone(),
                                    Type::Path(path_type(vec![
                                        segment(ident("std"), None),
                                        segment(ident("ffi"), None),
                                        segment(ident("c_void"), None),
                                    ])),
                                )),
                            },
                            FnArg::Typed(input) => BareFnArg {
                                attrs: Vec::new(),
                                name: None,
                                ty: map_type(&*input.ty),
                            },
                        })
                        .collect(),
                    variadic: None,
                    output: match &method.sig.output {
                        ReturnType::Default => ReturnType::Default,
                        ReturnType::Type(token, ty) => {
                            ReturnType::Type(token.clone(), Box::new(map_type(ty)))
                        }
                    },
                }),
            },
            item => panic!("{:?}", item),
        })
        .collect();

    let foreign_impl = impl_trait(
        name.clone(),
        path_type(vec![
            segment(ident("crate"), None),
            segment(ident("foreign"), None),
            segment(
                ident("Foreign"),
                Some(generics_argument(vec![GenericArgument::Type(
                    Type::TraitObject(TypeTraitObject {
                        dyn_token: Some(Token![dyn](name.span())),
                        bounds: punctuated(vec![TypeParamBound::Trait(TraitBound {
                            paren_token: None,
                            modifier: TraitBoundModifier::None,
                            lifetimes: None,
                            path: path(vec![segment(name.clone(), None)]),
                        })]),
                    }),
                )])),
            ),
        ]),
        &[],
        input
            .items
            .iter()
            .map(|item| match item {
                TraitItem::Method(method) => ImplItem::Method(ImplItemMethod {
                    attrs: Vec::new(),
                    vis: Visibility::Inherited,
                    defaultness: None,
                    sig: method.sig.clone(),
                    block: Block {
                        brace_token: Brace(method.sig.ident.span()),
                        stmts: vec![
                            Stmt::Expr(Expr::Verbatim({
                            let trait_name = name.clone();
                            let name = method.sig.ident.clone();
                            quote! {
                                log::trace!(concat!("Foreign::<", stringify!(#trait_name), ">::", stringify!(#name), " {:?}"), self.0);
                            }
                        })),
                        Stmt::Expr(Expr::Unsafe(ExprUnsafe {
                            attrs: Vec::new(),
                            unsafe_token: Token![unsafe](method.sig.ident.span()),
                            block: Block {
                                brace_token: Brace(method.sig.ident.span()),
                                stmts: vec![Stmt::Expr({
                                    let output = Expr::Call(ExprCall {
                                        attrs: Vec::new(),
                                        func: {
                                            let class_name = class_name.clone();
                                            let method = method.sig.ident.clone();
                                            Box::new(Expr::Verbatim(quote! {
                                                ((*(*(self.0 as *const #class_name<()>)).vtable).#method)
                                            }))
                                        },
                                        paren_token: Paren(method.sig.ident.span()),
                                        args: method
                                            .sig
                                            .inputs
                                            .iter()
                                            .map(|input| match input {
                                                FnArg::Receiver(input) => Expr::Cast(ExprCast {
                                                    attrs: Vec::new(),
                                                    expr: Box::new(Expr::Verbatim(quote! {
                                                        self.0
                                                    })),
                                                    as_token: Token![as](input.self_token.span),
                                                    ty: Box::new(Type::Ptr(pointer_type(
                                                        input.mutability.clone(),
                                                        Type::Path(path_type(vec![
                                                            segment(ident("std"), None),
                                                            segment(ident("ffi"), None),
                                                            segment(ident("c_void"), None),
                                                        ])),
                                                    ))),
                                                }),
                                                FnArg::Typed(input) => match &*input.pat {
                                                    Pat::Ident(id) => map_input(
                                                        Expr::Path(ExprPath {
                                                            attrs: Vec::new(),
                                                            qself: None,
                                                            path: path(vec![segment(
                                                                id.ident.clone(),
                                                                None,
                                                            )]),
                                                        }),
                                                        &input.ty,
                                                    ),
                                                    pat => panic!("{:?}", pat),
                                                },
                                            })
                                            .collect(),
                                    });

                                    match &method.sig.output {
                                        ReturnType::Default => output,
                                        ReturnType::Type(_, ty) => map_output(output, ty),
                                    }
                                })],
                            },
                        }))],
                    },
                }),
                item => panic!("{:?}", item),
            })
            .collect(),
    );

    let vtable_impl = vtable_impl(&input);

    let tokens = quote! {
        #input

        #vtable_impl

        #[repr(C)]
        pub(crate) struct #vtable_name {
            #vtable_fields
        }

        #[repr(C)]
        pub(crate) struct #class_name<T> {
            pub(crate) vtable: *const #vtable_name,
            pub(crate) instance: T,
        }

        #foreign_impl
    };

    tokens.into()
}
