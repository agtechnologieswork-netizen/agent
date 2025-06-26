import React from 'react';
import { Head, Link } from '@inertiajs/react';
import { PageProps } from '../types';

export default function Welcome({
    auth,
    canLogin,
    canRegister,
}: PageProps<{ canLogin: boolean; canRegister: boolean }>) {
    return (
        <>
            <Head title="Welcome" />
            <div className="bg-gray-50 text-black/50 dark:bg-black dark:text-white/50">
                <div className="relative min-h-screen flex flex-col items-center justify-center selection:bg-[#FF2D20] selection:text-white">
                    <div className="relative w-full max-w-2xl px-6 lg:max-w-7xl">
                        <header className="grid grid-cols-2 items-center gap-2 py-10 lg:grid-cols-3">
                            <div className="flex lg:justify-center lg:col-start-2">
                                <svg
                                    className="h-12 w-auto text-white lg:h-16 lg:text-[#FF2D20]"
                                    viewBox="0 0 62 65"
                                    fill="none"
                                    xmlns="http://www.w3.org/2000/svg"
                                >
                                    <path
                                        d="m61.1409 13.1409c0-2.4697-2.0012-4.4709-4.4709-4.4709-2.4697 0-4.4709 2.0012-4.4709 4.4709 0 2.4697 2.0012 4.4709 4.4709 4.4709 2.4697 0 4.4709-2.0012 4.4709-4.4709Z"
                                        fill="currentColor"
                                    />
                                    <path
                                        d="m56.67 17.6118c-2.4697 0-4.4709 2.0012-4.4709 4.4709 0 2.4697 2.0012 4.4709 4.4709 4.4709 2.4697 0 4.4709-2.0012 4.4709-4.4709 0-2.4697-2.0012-4.4709-4.4709-4.4709Z"
                                        fill="currentColor"
                                    />
                                </svg>
                            </div>
                            <nav className="-mx-3 flex flex-1 justify-end">
                                {auth.user ? (
                                    <Link
                                        href="/dashboard"
                                        className="rounded-md px-3 py-2 text-black ring-1 ring-transparent transition hover:text-black/70 focus:outline-none focus-visible:ring-[#FF2D20] dark:text-white dark:hover:text-white/80 dark:focus-visible:ring-white"
                                    >
                                        Dashboard
                                    </Link>
                                ) : (
                                    <>
                                        {canLogin && (
                                            <Link
                                                href="/login"
                                                className="rounded-md px-3 py-2 text-black ring-1 ring-transparent transition hover:text-black/70 focus:outline-none focus-visible:ring-[#FF2D20] dark:text-white dark:hover:text-white/80 dark:focus-visible:ring-white"
                                            >
                                                Log in
                                            </Link>
                                        )}

                                        {canRegister && (
                                            <Link
                                                href="/register"
                                                className="rounded-md px-3 py-2 text-black ring-1 ring-transparent transition hover:text-black/70 focus:outline-none focus-visible:ring-[#FF2D20] dark:text-white dark:hover:text-white/80 dark:focus-visible:ring-white"
                                            >
                                                Register
                                            </Link>
                                        )}
                                    </>
                                )}
                            </nav>
                        </header>

                        <main className="mt-6">
                            <div className="grid gap-6 lg:grid-cols-2 lg:gap-8">
                                <div className="flex flex-col items-start gap-6 overflow-hidden rounded-lg bg-white p-6 shadow-[0px_14px_34px_0px_rgba(0,0,0,0.08)] ring-1 ring-white/[0.05] transition duration-300 hover:text-black/70 hover:ring-black/20 focus:outline-none focus-visible:ring-[#FF2D20] md:row-span-3 lg:p-10 lg:pb-10 dark:bg-zinc-900 dark:ring-zinc-800 dark:hover:text-white/70 dark:hover:ring-zinc-700 dark:focus-visible:ring-[#FF2D20]">
                                    <div className="relative flex items-center gap-6 lg:items-end">
                                        <div id="docs-card-content" className="flex items-start gap-6 lg:flex-col">
                                            <div className="pt-3 sm:pt-5 lg:pt-0">
                                                <h2 className="text-xl font-semibold text-black dark:text-white">
                                                    Documentation
                                                </h2>

                                                <p className="mt-4 text-sm/relaxed">
                                                    Laravel has wonderful documentation covering every aspect of the
                                                    framework. Whether you are a newcomer or have prior experience with
                                                    Laravel, we recommend reading our documentation from beginning to end.
                                                </p>
                                            </div>
                                        </div>
                                    </div>
                                </div>
                            </div>
                        </main>

                        <footer className="py-16 text-center text-sm text-black dark:text-white/70">
                            Laravel Application Template
                        </footer>
                    </div>
                </div>
            </div>
        </>
    );
}
