import axios from "axios";
import md5 from "md5";
let api={}

const BASE_URL = "http://localhost:3000/"; // 本地
//const BASE_URL = "https://eplayer-server.vercel.app/"; // 线上
api = axios.create({

    withCredentials: false, 
    baseURL:BASE_URL+'api'
})

//user
export const getUser = (id) => api.get(`/user/${id}`)
export const updateUser = (id,payload) => api.put(`/user/${id}`,payload)
//export const updateUserInfo = (payload) => api.post(`/user`,payload)
export const createUser = (payload) => api.post(`/user`,payload)
export const loginUser = (id) => api.get(`/user/${id}`)
export const updateUserVersion = (id,payload) => api.put(`/user/${id}`,payload)
export const deleteUser = (id) => api.delete(`/user/${id}`)

//subtitle
export const getSubtitle = (md5) => api.get(`/subtitle/${md5}`)  //叫id还是md5都一样
export const createSubtitle = (payload) => api.post(`/subtitle`,payload)
export const deleteSubtitle = (id) => api.delete(`/subtitle/${id}`)
export const updateSubtitle = (id,payload) => api.put(`/subtitle/${id}`,payload)

const apis={
    getUser,
    updateUser,
    //updateUserInfo,
    createUser,
    loginUser,
    updateUserVersion,
    deleteUser,
    getSubtitle,
    createSubtitle,
    deleteSubtitle,
    updateSubtitle
}
export default apis