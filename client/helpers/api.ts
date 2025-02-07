/**
 * Web client for backpack API, only to be used with frontend
 */

import axios, { AxiosResponse } from "axios"

const BASE_URL = {
    toString: () => typeof window === "undefined" 
        ? process.env.API_URL 
        : window.location.origin + "/api"
}

enum UserRole {
    User = "user",
    Admin = "admin",
}

export type ThemeColor =
    | "gray"
    | "red"
    | "orange"
    | "yellow"
    | "green"
    | "teal"
    | "blue"
    | "cyan"
    | "purple"
    | "pink";

export interface AppInfo {
    appName: string;
    appDescription: string;
    smtp: boolean;
    color: ThemeColor;
    commit: string;
    inviteOnly: boolean;
    gitVersion: string;
}

export interface UserData {
    id: string;
    username: string;
    email: string;
    verified: boolean;
    role: UserRole;
}

export interface FileData {
    id: string;
    uploader: string;
    name: string;
    originalName: string;
    url: string;
    thumbnailUrl: string;
    hash: string;
    uploaded: Date;
    size: number;
}

export interface SearchResult<T> {
    page: number;
    pages: number;
    list: T[];
}

export interface UpdateUserSettings {
    email?: string;
    username?: string;
    newPassword?: string;
}

export interface ApplicationData {
    name: string;
    id: string;
    lastAccessed: string;
    userId: string;
    token?: string;
}

export const updateSettings = async (
    options: UpdateUserSettings,
    password: string
): Promise<UserData> => {
    console.log({
        ...options,
        currentPassword: password
    })
    return (
        await axios.put<UserData>(`${BASE_URL}/user/settings`, {
            ...options,
            currentPassword: password
        })
    ).data
}

export const uploadFile = async (file: File): Promise<FileData> => {
    const formData = new FormData()
    formData.append("uploadFile", file)

    return (
        await axios.post<FileData>(`${BASE_URL}/file`, formData, {
            headers: {
                "Content-Type": "multipart/form-data"
            }
        })
    ).data
}

export const getUsage = async (): Promise<number> => {
    return (await axios.get(`${BASE_URL}/file/stats`)).data.usage
}

export const searchFile = async (
    page: number,
    query?: string
): Promise<SearchResult<FileData>> => {
    const data = (
        await axios.get(
            `${BASE_URL}/file/list/${page}${query !== null ? "?query=" + query : ""}`
        )
    ).data
    for (const file of data.list) file.uploaded = new Date(file.uploaded)

    return data
}

export const getFile = async (fileId: string): Promise<FileData> => {
    return (await axios.get<FileData>(`${BASE_URL}/file/${fileId}`)).data
}

export const deleteFile = async (fileId: string): Promise<void> => {
    return await axios.delete(`${BASE_URL}/file/${fileId}`)
}

/**
 * Will log out of the service, httponly cookie will be deleted
 */
export const logout = async (): Promise<void> => {
    return await axios.post(`${BASE_URL}/auth/logout`)
}

/**
 * Log in with password authentication, key will be stored as httponly
 *
 * @param auth email
 * @param password password
 * @returns user data
 */
export const passwordLogin = async (
    auth: string,
    password: string
): Promise<UserData> => {
    return (
        await axios.post<UserData>(`${BASE_URL}/auth/basic`, {
            auth: auth,
            password: password
        })
    ).data
}

/**
 * Get data about the current user
 *
 * @returns user data
 */
export const getUserData = async (): Promise<UserData> => {
    return (await axios.get<UserData>(`${BASE_URL}/user`)).data
}

/**
 * Create and log in to a new account
 *
 * @param username username
 * @param email email
 * @param password password
 * @returns user data
 */
export const userCreate = async (
    username: string,
    email: string,
    password: string,
    registrationKey?: string
): Promise<UserData> => {
    return (
        await axios.post<UserData>(`${BASE_URL}/user`, {
            username: username,
            email: email,
            password: password,
            registrationKey: registrationKey
        })
    ).data
}

export const getAppInfo = async (): Promise<AppInfo> => {
    return (await axios.get<AppInfo>(`${BASE_URL}/info`)).data
}

export const verify = async (code: string): Promise<AxiosResponse<any>> => {
    return await axios.patch(`${BASE_URL}/user/verify/${code}`)
}

export const resendCode = async (): Promise<AxiosResponse<any>> => {
    return await axios.patch(`${BASE_URL}/user/verify/resend`)
}

export const getAllApplications = async (): Promise<ApplicationData[]> => {
    return (await axios.get<ApplicationData[]>(`${BASE_URL}/applications`)).data
}

export const getApplication = async (application_id: string): Promise<ApplicationData> => {
    return (await axios.get<ApplicationData>(`${BASE_URL}/applications/${application_id}`)).data     
}

export const getApplicationToken = async (application_id: string): Promise<{ token: string }> => {
    return (await axios.get<{ token: string }>(`${BASE_URL}/applications/${application_id}/token`)).data
}

export const applicationCreate = async (name: string): Promise<ApplicationData> => {
    return (await axios.post<ApplicationData>(`${BASE_URL}/applications`, {
        name: name
    })).data
}

export const deleteApplication = async (application_id: string): Promise<void> => {
    return await axios.delete(`${BASE_URL}/applications/${application_id}`)
}