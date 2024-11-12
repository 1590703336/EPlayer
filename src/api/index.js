import axios from "axios";
let api={}

const BASE_URL = "http://localhost:3000/";
api = axios.create({

    withCredentials: false,
    baseURL:BASE_URL+'api'
})

//user
export const updateUser = (payload) => api.put(`/user`,payload)
export const updateUserInfo = (payload) => api.post(`/user`,payload)



const apis={
    updateUser,
    updateUserInfo
}
export default apis