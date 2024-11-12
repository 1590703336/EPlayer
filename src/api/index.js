import axios from "axios";
let api={}

const BASE_URL = "http://localhost:3000/";
api = axios.create({

    withCredentials: false,
    baseURL:BASE_URL+'api'
})

//user
export const getUser = (id) => api.get(`/user/${id}`)
export const updateUser = (id,payload) => api.put(`/user/${id}`,payload)
export const updateUserInfo = (payload) => api.post(`/user`,payload)
export const loginUser = (id) => api.get(`/user/${id}`)
export const updateUserVersion = (id,payload) => api.put(`/user/${id}`,payload)

const apis={
    getUser,
    updateUser,
    updateUserInfo,
    loginUser,
    updateUserVersion
}
export default apis