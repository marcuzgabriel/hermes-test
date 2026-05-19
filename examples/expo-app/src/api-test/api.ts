// Simplified RTK Query API — mirrors real app's createApi pattern
import { createApi, fetchBaseQuery } from '@reduxjs/toolkit/query';

export interface LoginPayload {
  payload: string;
  signature: string;
}

export interface LoginResponse {
  accessToken: string;
  refreshToken: string;
  expiresIn: number;
}

export interface UserProfile {
  id: string;
  name: string;
  email: string;
}

const BASE_URL = 'https://api.example.com';

export const appApi = createApi({
  reducerPath: 'appApi',
  baseQuery: fetchBaseQuery({ baseUrl: BASE_URL }),
  tagTypes: ['User'],
  endpoints: (builder) => ({
    login: builder.mutation<LoginResponse, LoginPayload>({
      query: (payload) => ({
        url: '/auth/login',
        method: 'POST',
        body: payload,
      }),
    }),
    getProfile: builder.query<UserProfile, string>({
      query: (userId) => `/users/${userId}`,
      providesTags: ['User'],
    }),
    updateProfile: builder.mutation<UserProfile, { id: string; name: string }>({
      query: ({ id, ...body }) => ({
        url: `/users/${id}`,
        method: 'PUT',
        body,
      }),
      invalidatesTags: ['User'],
    }),
  }),
});

export default appApi;
